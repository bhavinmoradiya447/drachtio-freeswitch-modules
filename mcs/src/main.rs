use tokio::sync::mpsc;
use warp::http::Uri;
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use warp::Filter;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_client::MultiCastServiceClient;
use crate::mcs::Payload;

struct UuidClientChannel {
    sink: mpsc::UnboundedSender<Payload>,
}

impl UuidClientChannel {
    fn new(
        address: String,
        mut client: MultiCastServiceClient<tonic::transport::Channel>,
    ) -> Self {
        let (sink, mut stream) = mpsc::unbounded_channel::<Payload>();
        tokio::spawn(async move {
            let payload_stream = async_stream::stream! {
                while let Some(chunk) = stream.recv().await {
                    //println!("[trace] sending uuid: {}, seq: {} to: {}", chunk.uuid, chunk.seq, address);
                    yield chunk;
                }
            };
            let request = tonic::Request::new(payload_stream);
            let _response = client.listen(request).await.unwrap();
        });
        UuidClientChannel {
            sink
        }
    }
}

struct AddressChannel {
    //client: MultiCastServiceClient<tonic::transport::Channel>,
    sink: mpsc::UnboundedSender<Payload>,
    uuid_client_channels: Arc<Mutex<HashMap<String, UuidClientChannel>>>,
}

impl AddressChannel {
    async fn new(
        address: String,
        address_uri: Uri,
        uuid: String,
    ) -> Self {
        // let uuid_client_channels = HashMap::new();
        let client = MultiCastServiceClient::connect(address_uri).await.unwrap();
        let (sink, mut stream) = mpsc::unbounded_channel::<Payload>();
        let uuid_client_channels = Arc::new(Mutex::new(HashMap::new()));
        let uuid_client_channels1 = uuid_client_channels.clone();
        {
            let uuid_client_channel = UuidClientChannel::new(address.clone(), client);
            let mut uuid_client_channels = uuid_client_channels.lock().await;
            uuid_client_channels.insert(uuid.clone(), uuid_client_channel);
        }

        tokio::spawn(async move {
            while let Some(chunk) = stream.recv().await {
                let mut uuid_client_channels = uuid_client_channels.lock().await;
                for (uuid, uuid_client_channel) in uuid_client_channels.iter() {
                    if uuid == &chunk.uuid {
                        //println!("[trace] sending uuid: {}, seq: {} to: {}", chunk.uuid, chunk.seq, address);
                        let _ = uuid_client_channel.sink.send(chunk.clone());
                    }
                }
                if chunk.size == 0 {
                    println!("[debug] closing uuid: {} for {}", chunk.uuid, address);
                    uuid_client_channels.remove(chunk.uuid.as_str());
                }
            }
        });
        AddressChannel {
            //client: client,
            sink: sink,
            uuid_client_channels: uuid_client_channels1,
        }
    }
}


#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
// #[tokio::main]
async fn main() -> Result<(), Error> {
    let address_to_channel = Arc::new(Mutex::new(HashMap::new()));

    let start_cast = warp::path!("start_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_address_to_channel(address_to_channel.clone()))
        .and_then(start_cast_handler);

    // start server in a new thread
    tokio::spawn(async move {
        println!("[info] starting http server");
        warp::serve(start_cast).run(([127, 0, 0, 1], 3030)).await;
    });

    let mut fd = open_named_pipe().unwrap();
    //let mut header_buf = [0; 16 + 4 + 8 + 4];
    let mut data =  vec![];
    loop {
        //let ret = fd.read(&mut header_buf);
        let mut new_data = vec![];
        let ret = fd.read_to_end(&mut new_data);
        println!("Reading Named pipe done size : {}", new_data.len());
        match ret {
            Ok(n) if n == 0 => {
                // println!("[trace] unable to read header bytes: {}", n);
                // sleep for 2 milliseconds
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                // yield to other tasks
                // tokio::task::yield_now().await;
                drop(new_data);
                continue;
                // return Err(Error::new(ErrorKind::Other, "Unable to read header bytes"));
            }

            Ok(n) if n > 0 => {
                 data.append(&mut new_data);
                 drop(new_data);
            }
            Err(e) => {
                println!(   "[error] unable to read header bytes: {}", e);
                // sleep for 2 milliseconds
                drop(new_data);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                // yield to other tasks
                // tokio::task::yield_now().await;
                continue;
            }
            _ => {}
        }

        while data.len() >= 32 {

            let mut remaining = data.split_off(32);
            //let mut header = Vec::new();
            //header.append(&mut data);
            let mut header_copy = data.clone();
            let header_buf: [u8; 32] = data.clone().try_into().unwrap();
                //header.try_into().unwrap_or_else(|v: Vec<u8>| panic!("Expected a Vec of length {} but it was {}", 32, v.len()));
            let mut payload = parse_header(&header_buf)?;
            let size = payload.size as usize;
            println!("uuid: {}, seq: {}, size: {}, remaining {}", payload.uuid, payload.seq, payload.size, remaining.len());
            if remaining.len() < size  {
                data = vec![];
                data.append(&mut header_copy);
                data.append(&mut remaining);
                break;
            }

            let current_system_time = SystemTime::now();
            let duration_since_epoch = current_system_time.duration_since(UNIX_EPOCH).unwrap();
            let milliseconds_timestamp = duration_since_epoch.as_millis() as u64;
            if size != 0 {
                if size == remaining.len() {
                    let mut payload_buf = vec![];
                    payload_buf.append(&mut remaining);
                    payload.audio = payload_buf;
                    data = vec![];
                    break;
                } else {
                    let mut rest_data = remaining.split_off(size);
                    payload.audio = remaining;
                    data = vec![];
                    data.append(&mut rest_data);
                }
                println!("[trace] sending payload for {} with delay {}", payload.uuid, (milliseconds_timestamp - payload.timestamp));
            } else {
                println!("[trace] Got end of stream for uuid: {} with delay {}", payload.uuid, (milliseconds_timestamp - payload.timestamp));
                data = vec![];
                data.append(&mut remaining);
            }
        }



        // for each sink in address_to_sink send payload
        /*  let mut address_to_channel = address_to_channel.lock().await;
          for (address, address_channel) in address_to_channel.iter_mut() {
              //println!("[trace] sending payload to sink with address: {} ", address);
              //let _ = address_channel.sink.send(payload.clone());
          }

         */
    }
}

async fn start_cast_handler(
    body: HashMap<String, String>,
    address_to_channel: Arc<Mutex<HashMap<String, AddressChannel>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = body.get("uuid").unwrap().clone();
    let address = body.get("address").unwrap().clone();
    println!("[info] start-cast for uuid: {}, address: {}", uuid, address);

    /*  let address_uri: Uri = address.parse().unwrap();
      let mut address_to_channel = address_to_channel.lock().await;
      if address_to_channel.contains_key(&address) {
          let address_channel = address_to_channel.get(&address.clone()).unwrap();
          let client = MultiCastServiceClient::connect(address_uri).await.unwrap();
          address_channel.uuid_client_channels.lock().await.insert(uuid.clone(), UuidClientChannel::new(address.clone(), client));
      } else {
          let address_channel = AddressChannel::new(address.clone(), address_uri, uuid.clone()).await;
          address_to_channel.insert(address.clone(), address_channel);
      }*/

    Ok(warp::reply::json(&"ok"))
}

fn with_address_to_channel(
    address_to_channel: Arc<Mutex<HashMap<String, AddressChannel>>>,
) -> impl warp::Filter<
    Extract=(Arc<Mutex<HashMap<String, AddressChannel>>>, ),
    Error=std::convert::Infallible,
> + Clone {
    warp::any().map(move || address_to_channel.clone())
}


fn open_named_pipe() -> Result<File, Error> {
    // Open the named pipe in read mode
    let pipe_path = "/tmp/mod-audio-cast-pipe";
    println!("[info] opening named pipe: {}", pipe_path);
    let fd = match OpenOptions::new().read(true).open(pipe_path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Create the named pipe if it doesn't exist
            Command::new("mkfifo")
                .arg(pipe_path)
                .output()
                .expect("[error] Failed to create named pipe");
            // Try opening the pipe again
            match OpenOptions::new().read(true).open(pipe_path) {
                Ok(file) => file,
                Err(e) => {
                    println!("[error] unable to open named pipe: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            println!("[error] unable to open named pipe: {}", e);
            return Err(e);
        }
    };
    println!("[info] named pipe: {} is opened now", pipe_path);
    Ok(fd)
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
