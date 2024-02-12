use tokio::sync::{broadcast, mpsc};
use warp::http::Uri;

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use uuid::Uuid;
use warp::Filter;

use tracing::{error, info, instrument, trace};
use tracing_subscriber;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_client::MultiCastServiceClient;
use crate::mcs::Payload;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let address_client = Arc::new(Mutex::new(HashMap::new()));
    let uuid_channel = Arc::new(Mutex::new(HashMap::new()));
    let with_address_client = warp::any().map(move || Arc::clone(&address_client));
    let with_uuid_channel = warp::any().map(move || Arc::clone(&uuid_channel));

    let start_cast = warp::path!("start_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_address_client.clone())
        .and(with_uuid_channel.clone())
        .and_then(start_cast_handler);

    tokio::spawn(async move {
        println!("starting http server");
        warp::serve(start_cast).run(([127, 0, 0, 1], 3030)).await;
    }).await.expect("Panic");
}

#[instrument]
async fn start_cast_handler(
    body: HashMap<String, String>,
    address_client: Arc<Mutex<HashMap<String, MultiCastServiceClient<tonic::transport::Channel>>>>,
    uuid_channel: Arc<Mutex<HashMap<String, UnboundedSender<Payload>>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = body.get("uuid").unwrap().clone();
    let address = body.get("address").unwrap().clone();
    info!("Starting cast for uuid: {}, address: {}", uuid, address);

    /*let mut address_client = address_client.lock().await;
    let mut client = match address_client.get(&address) {
        Some(client) => client.clone(),
        None => {
            let client = connect(address.clone()).await.unwrap();
            address_client.insert(address.clone(), client.clone());
            client
        }
    };*/
    // let mut uuid_channel = uuid_channel.lock().await;
    // let receiver = match uuid_channel.get(&uuid) {
    //     Some(sender) => sender.clone(),
    //     None => {
    //let (sender, mut receiver) = mpsc::unbounded_channel::<Payload>();
    //drop(rx);
    //let sender_clone = sender.clone();
    //uuid_channel.insert(uuid.clone(), sender.clone());
    tokio::spawn(async move {
        let mut file = open_named_pipe(uuid.clone()).unwrap();
        let mut header_buf = [0; 16 + 4 + 8 + 4];
        loop {
            let ret = file.read(&mut header_buf);
            match ret {
                Ok(n) if n == 0 => {
                    // println!("[trace] unable to read header bytes: {}", n);
                    // sleep for 2 milliseconds
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    // yield to other tasks
                    tokio::task::yield_now().await;
                    continue;
                    // return Err(Error::new(ErrorKind::Other, "Unable to read header bytes"));
                }
                Err(e) => {
                    println!("[error] unable to read header bytes: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    // yield to other tasks
                    tokio::task::yield_now().await;
                    continue;
                }
                _ => {}
            }

            let mut payload = parse_header(&header_buf).unwrap();
            let size = payload.size;

            if size != 0 {
                let mut payload_buf = vec![0; size as usize];
                let _ = file.read_exact(&mut payload_buf);
                payload.audio = payload_buf;
            }

            let current_system_time = SystemTime::now();
            let duration_since_epoch = current_system_time.duration_since(UNIX_EPOCH).unwrap();
            let milliseconds_timestamp = duration_since_epoch.as_millis() as u64;

            info!("sending uuid: {}, seq: {} delay: {}", payload.uuid, payload.seq, (milliseconds_timestamp - payload.timestamp) );

            if size == 0 {
                close_named_pipe(uuid.clone()).unwrap();
                break;
            }
        }
    });
    //receiver
    //}
    //};

//    let mut receiver = sender.subscribe();
    /* tokio::spawn(async move {
         let payload_stream = async_stream::stream! {
             while let Ok(chunk) = receiver.recv().await {
                 info!("sending uuid: {}, seq: {} to: {}", chunk.uuid, chunk.seq, address);
                 yield chunk;
             }
         };
         //let request = tonic::Request::new(payload_stream);
         //let _response = client.listen(request).await.unwrap();
     });*/

    Ok(warp::reply::json(&"ok"))
}

fn parse_header(header_buf: &[u8]) -> Result<Payload, Error> {
    let mut pos = 0;
    let mut id = [0; 16];
    id.copy_from_slice(&header_buf[pos..pos + 16]);
    pos += 16;
    let uuid = Uuid::from_bytes(id).to_string();
    let seq = u32::from_ne_bytes(header_buf[pos..pos + 4].try_into().unwrap());
    pos += 4;
    let timestamp = u64::from_ne_bytes(header_buf[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let size = u32::from_ne_bytes(header_buf[pos..pos + 4].try_into().unwrap());

    let payload = Payload {
        uuid: uuid.clone(),
        seq: seq,
        timestamp: timestamp,
        size: size,
        audio: vec![],
    };
    // println!("[trace] uuid: {}, seq: {}, size: {}", uuid, seq, size);
    Ok(payload)
}

// method to parse payload from buffer
fn parse_payload(buf: &[u8]) -> Result<Payload, Error> {
    let mut payload = Payload::default();
    payload.uuid = Uuid::from_slice(&buf[0..16]).unwrap().to_string();
    payload.seq = u32::from_ne_bytes(buf[16..20].try_into().unwrap());
    payload.timestamp = u64::from_ne_bytes(buf[20..28].try_into().unwrap());
    payload.size = u32::from_ne_bytes(buf[28..32].try_into().unwrap());
    payload.audio = buf[32..].to_vec();
    Ok(payload)
}

async fn connect(
    address: String,
) -> Result<MultiCastServiceClient<tonic::transport::Channel>, Error> {
    let uri = match address.parse::<Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            error!("Failed to parse uri: {}", e);
            return Err(Error::new(ErrorKind::InvalidInput, "Failed to parse uri"));
        }
    };
    let client = MultiCastServiceClient::connect(uri).await.unwrap();
    Ok(client)
}

fn close_named_pipe(uuid: String) -> Result<(), Error> {
    let pipe_path = format!("/tmp/mod-audio-cast-pipes/{}", uuid);
    info!("closing named pipe: {}", pipe_path);
    // delete the named pipe
    Command::new("rm")
        .arg(pipe_path)
        .output()
        .expect("[error] Failed to delete named pipe");
    Ok(())
}

fn open_named_pipe(uuid: String) -> Result<File, Error> {
    // Open the named pipe /tmp/mod-audio-cast-pipes/{uuid} in read mode
    let pipe_path = format!("/tmp/mod-audio-cast-pipes/{}", uuid);
    info!("opening named pipe: {}", pipe_path);

    Command::new("mkfifo")
        .arg(pipe_path.clone())
        .output()
        .expect("[error] Failed to create named pipe");
    let fd = match OpenOptions::new().read(true).open(pipe_path.clone()) {
        Ok(file) => file,
        Err(e) => {
            error!("unable to open named pipe: {}", e);
            return Err(e);
        }
    };
    Ok(fd)
}