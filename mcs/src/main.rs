use tokio::sync::broadcast;
use warp::http::Uri;

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::{fs, io, thread};
use std::io::{BufReader, Error};
use std::io::ErrorKind;
use std::io::Read;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::pin;
use tokio::sync::Mutex;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;
use warp::Filter;

use tracing::{error, info, instrument, trace};
use tracing_subscriber;
use unix_named_pipe::FileFIFOExt;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_client::MultiCastServiceClient;
use crate::mcs::Payload;

const PIPE_DIR: &str = "/tmp/mod-audio-cast-pipes";

#[tokio::main(flavor = "multi_thread", worker_threads = 100)]
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

    let mut address_client = address_client.lock().await;
    let mut client = connect(address.clone()).await.unwrap();
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

    if new_sender {
        let uuid1 = uuid.clone();
        //create_named_pipe(uuid.clone()).unwrap();
        tokio::spawn(async move {
            let fd = open_named_pipe(uuid.clone()).unwrap();

            let mut file_stream = FileReaderStream::new(BufReader::new(fd))
                .throttle(Duration::from_millis(100));
            pin!(file_stream);
            while let Some(item) = file_stream.next().await {
                for payload in item {
                    let uuid = payload.uuid.clone();
                    let seq = payload.seq.clone();
                    match sender.send(payload) {
                        Ok(n) => trace!("pushed seq {} for call leg {}",
                        seq,
                        uuid ),
                        Err(err) => error!(
                            "Failed to push seq {} for call leg {}, error {}",seq,uuid, err)
                    }
                }
            }
            // sleep for 5 ms
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            //info!("closing named pipe for uuid: {}, delay: 100+ms : {} , 500+ms: {}, 1+Sec: {}, 2+Sec: {}, 3+Sec {}", uuid.clone(),
            // ms_100, ms_500, ms_1000, ms_2000, ms_3000);
            info!("Closing named pipe for uuid: {}", uuid1);

            close_named_pipe(uuid.clone()).unwrap();
            // remove uuid from uuid channel list
            let mut uuid_channel1 = uuid_channel1.lock().await;
            uuid_channel1.remove(&uuid1);
            // close broadcast channel for this uuid
            drop(sender);
            drop(client2);
        });
    };

    info!("replying ok");
    Ok(warp::reply::json(&"ok"))
}
/*
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

 */

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
    fs::remove_file(&pipe_path).expect(&*format!("could not remove pipe file {}", pipe_path));
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

    let pipe = unix_named_pipe::open_read(&pipe_path);
    if let Err(err) = pipe {
        return match err.kind() {
            ErrorKind::NotFound => {
                info!("creating pipe at: {:?}", pipe_path);
                unix_named_pipe::create(&pipe_path, Some(0o660))?;

                // Note that this has the possibility to recurse forever if creation `open_write`
                // fails repeatedly with `io::ErrorKind::NotFound`, which is certainly not nice behaviour.
                open_named_pipe(uuid)
            }
            _ => {
                Err(err)
            }
        };
    }

    let pipe_file = pipe.unwrap();
    let is_fifo = pipe_file
        .is_fifo()
        .expect("could not read type of file at pipe path");
    if !is_fifo {
        return Err(Error::new(
            ErrorKind::Other,
            format!(
                "expected file at {:?} to be fifo, is actually {:?}",
                pipe_path,
                pipe_file.metadata()?.file_type(),
            ),
        ));
    }

    Ok(pipe_file)
}


#[derive(Debug)]
pub struct FileReaderStream {
    reader: BufReader<File>,
    data: Vec<u8>,
    done: bool,
    ms_200: usize,
    ms_500: usize,
    ms_1000: usize,
    ms_2000: usize,
    ms_3000: usize,

}

impl FileReaderStream {
    pub fn new(file_buffer: BufReader<File>) -> Self {
        Self { reader: file_buffer, data: vec![], done: false, ms_200: 0, ms_500: 0, ms_1000: 0, ms_2000: 0, ms_3000: 0 }
    }
}

impl Stream for FileReaderStream {
    type Item = Vec<Payload>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.done {
            true => Poll::Ready(None),
            false => {
                let mut new_data = vec![];
                let res = self.reader.read_to_end(&mut new_data);
                return match res {
                    Err(err) => match err.kind() {
                        ErrorKind::WouldBlock => {
                            let waker = cx.waker().clone();
                            thread::spawn(move || {
                                thread::sleep(Duration::from_millis(20));
                                waker.wake();
                            });
                            info!("Block for data to be available");
                            drop(new_data);
                            Poll::Pending
                        }
                        _ => {
                            error!("error while reading from pipe: {:?}", err);
                            drop(new_data);
                            Poll::Ready(None)
                        }
                    }
                    Ok(n) => if n == 0 {
                        info!("Got Empty data");
                        let waker = cx.waker().clone();
                        thread::spawn(move || {
                            thread::sleep(Duration::from_millis(20));
                            waker.wake();
                        });
                        drop(new_data);
                        Poll::Pending
                    } else {
                        // let payload: Message = json::from_str(&line).expect("could not deserialize line");
                        // if n > 67200 {
                        let mut sender = vec![];
                        self.data.append(&mut new_data);
                        drop(new_data);
                        trace!("Got Data with Size {}, data len: {}", n, self.data.len());
                        while self.data.len() >= 32 {
                            let mut remaining = self.data.split_off(32);
                            //let mut header = Vec::new();
                            //header.append(&mut data);
                            let mut header_copy = self.data.clone();
                            let header_buf: [u8; 32] = self.data.clone().try_into().unwrap();
                            //header.try_into().unwrap_or_else(|v: Vec<u8>| panic!("Expected a Vec of length {} but it was {}", 32, v.len()));
                            let mut payload = parse_header(&header_buf).unwrap();
                            let size = payload.size as usize;
                            //println!("uuid: {}, seq: {}, size: {}, remaining {}", payload.uuid, payload.seq, payload.size, remaining.len());
                            if remaining.len() < size {
                                self.data = vec![];
                                self.data.append(&mut header_copy);
                                self.data.append(&mut remaining);
                                break;
                            }

                            let current_system_time = SystemTime::now();
                            let duration_since_epoch = current_system_time.duration_since(UNIX_EPOCH).unwrap();
                            let milliseconds_timestamp = duration_since_epoch.as_millis() as u64;
                            let uuid = payload.uuid.clone();
                            trace!("[trace] sending payload for {} , seq {}, size {}, with delay {}",
                            uuid, payload.seq, payload.size, (milliseconds_timestamp - payload.timestamp));
                            let delay = milliseconds_timestamp - payload.timestamp;
                            if delay > 200 && delay < 500 {
                                self.ms_200 += 1;
                            } else if delay > 500 && delay < 1000 {
                                self.ms_500 += 1;
                            } else if delay > 1000 && delay < 2000 {
                                self.ms_1000 += 1;
                            } else if delay > 2000 && delay < 3000 {
                                self.ms_2000 += 1;
                            } else if delay > 2000 {
                                self.ms_3000 += 1;
                            }

                            if size != 0 {
                                if size == remaining.len() {
                                    let mut payload_buf = vec![];
                                    payload_buf.append(&mut remaining);
                                    payload.audio = payload_buf;
                                    self.data = vec![];
                                    break;
                                } else {
                                    let mut rest_data = remaining.split_off(size);
                                    payload.audio = remaining;
                                    self.data = vec![];
                                    self.data.append(&mut rest_data);
                                }
                                sender.push(payload);
                            } else {
                                info!("[trace] Delay for uuid {} : 200+ms: {}, 500+ms: {}, 1+Sec: {}, 2+Sec: {}, 3+Sec: {}",
                                uuid, self.ms_200, self.ms_500, self.ms_1000, self.ms_2000, self.ms_3000);
                                sender.push(payload);
                                self.done = true;
                                break;
                            }
                        }
                        Poll::Ready(Some(sender))
                    }
                };
            }
        }
    }
}
