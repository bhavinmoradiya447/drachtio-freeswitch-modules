use tokio::sync::broadcast;
use warp::http::Uri;

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
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

const PIPE_DIR: &str = "/tmp/mod-audio-cast-pipes";

#[tokio::main(flavor = "multi_thread", worker_threads = 50)]
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

    info!("starting http server");
    warp::serve(start_cast).run(([127, 0, 0, 1], 3030)).await;
}

//#[instrument]
async fn start_cast_handler(
    body: HashMap<String, String>,
    address_client: Arc<Mutex<HashMap<String, MultiCastServiceClient<tonic::transport::Channel>>>>,
    uuid_channel: Arc<Mutex<HashMap<String, broadcast::Sender<Payload>>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = body.get("uuid").unwrap().clone();
    let address = body.get("address").unwrap().clone();
    info!("Starting cast for uuid: {}, address: {}", uuid, address);

    /*let mut address_client = address_client.lock().await;
    let mut client =  connect(address.clone()).await.unwrap();
    let mut client2 = client.clone();
   /* let mut client = match address_client.get(&address) {
        Some(client) => client.clone(),
        None => {
            info!("connecting to address: {}", address);
            let client = connect(address.clone()).await.unwrap();
            address_client.insert(address.clone(), client.clone());
            client
        }
    };*/
    let uuid_channel1 = uuid_channel.clone();
    let mut uuid_channel = uuid_channel.lock().await;
    let mut new_sender = false;

    let sender = match uuid_channel.get(&uuid) {
        Some(sender) => sender.clone(),
        None => {
            info!("creating broadcast channel for uuid: {}", uuid);
            // let (sender, rx) = broadcast::channel::<Payload>(1000);
            // create unbounded broadcast channel
            let (sender, rx) = broadcast::channel(3000);
            drop(rx);
            uuid_channel.insert(uuid.clone(), sender.clone());
            new_sender = true;
            sender
        }
    };
    let sender1 = sender.clone();
    let mut receiver = sender1.subscribe();
    info!("sending payload to address: {}", address);
    let uuid_clone = uuid.clone();

    tokio::spawn(async move {
        info!("initialising payload stream");
        let payload_stream = async_stream::stream! {
            while let Ok(chunk) = receiver.recv().await {
                trace!("sending uuid: {}, seq: {}, size:{}, to: {}", chunk.uuid, chunk.seq, chunk.size, address);
                yield chunk;
            }
            info!("done sending {} to {}", uuid_clone.clone(), address);
        };
        let request = tonic::Request::new(payload_stream);
        let _response = client.listen(request).await.unwrap();

    });

    if new_sender {*/
    let uuid1 = uuid.clone();
    create_named_pipe(uuid.clone()).unwrap();
    tokio::spawn(async move {
        let mut fd = open_named_pipe(uuid.clone()).unwrap();
        let mut buf = [0; 336000];
        let mut remaining = 0;
        let mut done = false;
        let mut ms_100 = 0;
        let mut ms_500 = 0;
        let mut ms_1000 = 0;
        let mut ms_2000 = 0;
        let mut ms_3000 = 0;

        while !done {
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            tokio::task::yield_now().await;
            let ret = fd.read(&mut buf[remaining..]);
            match ret {
                Ok(n) if n == 0 => {
                    // sleep for 2 milliseconds
                    //std::thread::sleep(std::time::Duration::from_millis(2));
                    // yield to other tasks
                    //tokio::task::yield_now().await;
                    continue;
                }
                // if size is greater than 0, parse the header
                Ok(n) if n > 0 => {
                     if n > 67200 {
                         info!("read {} bytes", n);
                     }
                    let mut pos = 0;
                    loop {
                        let (payload, new_pos) = parse_payload(&buf, pos, n);
                        if new_pos == 0 {
                            remaining = n - pos;
                            //info!("remaining: {}", remaining);
                            // copy the remaining bytes to a separate slice
                            let mut remaining_buf = [0; 336000];
                            remaining_buf[..remaining].copy_from_slice(&buf[pos..n]);
                            buf[..remaining].copy_from_slice(&remaining_buf[..remaining]);
                            break;
                        }
                        let size = payload.size;
                        let current_system_time = SystemTime::now();
                        let duration_since_epoch = current_system_time.duration_since(UNIX_EPOCH).unwrap();
                        let milliseconds_timestamp = duration_since_epoch.as_millis() as u64;
                        //println!("[trace] sending payload for {} with delay {}", payload.uuid, (milliseconds_timestamp - payload.timestamp));
                        let delay = milliseconds_timestamp - payload.timestamp;
                        sender.send(payload).unwrap();
                        if delay > 100 && delay < 500 {
                            ms_100 += 1;
                        } else if delay > 500 && delay < 1000 { ms_500 += 1; } else if delay > 1000 && delay < 2000 { ms_1000 += 1; } else if delay > 2000 && delay < 3000 { ms_2000 += 1; } else if delay > 2000 { ms_3000 += 1; }
                        if size == 0 {
                            done = true;
                            break;
                        }
                        if new_pos == n {
                            remaining = 0;
                            break;
                        }
                        pos = new_pos;
                    }
                }
                Err(e) => {
                    error!("unable to read header bytes: {}", e);
                    break;
                    // return Err(e);
                }
                _ => {}
            }
        }
        // sleep for 5 ms
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        drop(fd);
        info!("closing named pipe for uuid: {}, delay: 100+ms : {} , 500+ms: {}, 1+Sec: {}, 2+Sec: {}, 3+Sec {}", uuid.clone(),
        ms_100, ms_500, ms_1000, ms_2000, ms_3000);
        close_named_pipe(uuid.clone()).unwrap();
        // remove uuid from uuid channel list
        /*let mut uuid_channel1 = uuid_channel1.lock().await;
         uuid_channel1.remove(&uuid1);
         // close broadcast channel for this uuid
         drop(sender);
         drop(client2);*/
    });
    //};

    info!("replying ok");
    Ok(warp::reply::json(&"ok"))
}

fn parse_header(buf: &[u8], start_pos: usize) -> Result<Payload, Error> {
    trace!("parsing header from: {} ", start_pos);
    let mut payload = Payload::default();
    payload.uuid = Uuid::from_slice(&buf[start_pos..start_pos + 16]).unwrap().to_string();
    payload.seq = u32::from_ne_bytes(buf[start_pos + 16..start_pos + 20].try_into().unwrap());
    payload.timestamp = u64::from_ne_bytes(buf[start_pos + 20..start_pos + 28].try_into().unwrap());
    payload.size = u32::from_ne_bytes(buf[start_pos + 28..start_pos + 32].try_into().unwrap());
    Ok(payload)
}

fn parse_payload(buf: &[u8], start_pos: usize, end_pos: usize) -> (Payload, usize) {
    if (end_pos - start_pos) < 32 {
        return (Payload {
            uuid: "".to_string(),
            seq: 0,
            timestamp: 0,
            size: 0,
            audio: vec![],
        }, 0);
    }
    // use parse_header to parse the header
    let mut payload = parse_header(buf, start_pos).unwrap();
    let new_pos = start_pos + 32 + payload.size as usize;
    if new_pos > end_pos {
        return (Payload {
            uuid: "".to_string(),
            seq: 0,
            timestamp: 0,
            size: 0,
            audio: vec![],
        }, 0);
    }
    payload.audio = buf[start_pos + 32..payload.size as usize + start_pos + 32].to_vec();
    trace!("parsed payload: uuid: {}, seq: {}, size: {}, new_pos: {}", payload.uuid, payload.seq, payload.size, new_pos);
    (payload, new_pos)
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
    let pipe_path = format!("{}/{}", PIPE_DIR, uuid);
    info!("closing named pipe: {}", pipe_path);
    // delete the named pipe
    Command::new("rm")
        .arg(pipe_path)
        .output()
        .expect("[error] Failed to delete named pipe");
    Ok(())
}

// method to create named pipe
fn create_named_pipe(uuid: String) -> Result<(), Error> {
    let pipe_path = format!("{}/{}", PIPE_DIR, uuid);
    info!("creating named pipe: {}", pipe_path);
    // create the named pipe
    Command::new("mkfifo")
        .arg(pipe_path)
        .output()
        .expect("[error] Failed to create named pipe");
    Ok(())
}

fn open_named_pipe(uuid: String) -> Result<File, Error> {
    // Open the named pipe /tmp/mod-audio-cast-pipes/{uuid} in read mode
    let pipe_path = format!("{}/{}", PIPE_DIR, uuid);
    info!("opening named pipe: {}", pipe_path);
    let fd = match OpenOptions::new().read(true).open(pipe_path.clone()) {
        Ok(file) => file,
        Err(e) => {
            error!("unable to open named pipe: {}", e);
            return Err(e);
        }
    };
    Ok(fd)
}
