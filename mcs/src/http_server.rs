use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use tonic::transport::{Channel, Uri};
use tracing::{info, instrument};
use warp::Filter;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_client::MultiCastServiceClient;
use crate::mcs::Payload;
use crate::mcs::PayloadType;
use crate::AddressPayload;
use crate::UuidChannels;

const GRPC_CONNECTIONS: usize = 20;

#[derive(Debug, Default)]
struct AddressClients {
    clients: HashMap<String, MultiCastServiceClient<Channel>>,
}
static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub async fn start_http_server(
    uuid_channels: Arc<Mutex<UuidChannels>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let with_uuid_channel = warp::any().map(move || Arc::clone(&uuid_channels));

    let address_client = Arc::new(Mutex::new(AddressClients::default()));
    let with_address_client = warp::any().map(move || Arc::clone(&address_client));

    let start_cast = warp::path!("start_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel.clone())
        .and(with_address_client.clone())
        .and_then(start_cast_handler);

    let stop_cast = warp::path!("stop_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel.clone())
        .and_then(stop_cast_handler);

    let dispatch_event = warp::path!("dispatch_event")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel)
        .and_then(dispatch_event_handler);

    info!("starting http server");
    warp::serve(start_cast.or(stop_cast).or(dispatch_event))
        .run(([127, 0, 0, 1], 3030))
        .await;
    Ok(())
}

#[instrument(name = "start_cast", skip(channels, address_client))]
async fn start_cast_handler(
    body: HashMap<String, String>,
    channels: Arc<Mutex<UuidChannels>>,
    address_client: Arc<Mutex<AddressClients>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let count = COUNTER.fetch_add(1, SeqCst);
    let uuid = body.get("uuid").unwrap().clone();
    let uuid_clone = uuid.clone();
    let address = body.get("address").unwrap().clone();
    // get mode or set to default "combined"
    let mode = body.get("mode").unwrap_or(&"combined".to_string()).clone();
    // get metadat or set to empty string
    let metadata = body.get("metadata").unwrap_or(&"".to_string()).clone();
    let group = (count / GRPC_CONNECTIONS) % GRPC_CONNECTIONS;
    let address_key = format!("{}-{}", address, group);
    let address_uri = Uri::try_from(&address).unwrap();

    let mut address_client = address_client.lock().unwrap();
    let mut client = match address_client.clients.get(&address_key) {
        Some(client) => client.clone(),
        None => {
            let grpc_channel = Channel::builder(address_uri).connect_lazy();
            let grpc_client = MultiCastServiceClient::new(grpc_channel);
            address_client
                .clients
                .insert(address_key, grpc_client.clone());
            grpc_client
        }
    };

    let mut channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&uuid.clone()) {
        Some(channel) => channel.clone(),
        None => {
            info!("creating channel in http_server for uuid: {}", uuid.clone());
            let (tx, _) = broadcast::channel(1000);
            channels.uuid_sender_map.insert(uuid.clone(), tx.clone());
            tx
        }
    };

    let mut receiver = channel.subscribe();
    tokio::spawn(async move {
        info!("initialising payload stream for uuid: {} to: {}", uuid, address);
        let payload_stream = async_stream::stream! {
            let mut done = false;
            while let Ok(mut addr_payload) = receiver.recv().await {
                if addr_payload.payload.payload_type == <PayloadType as Into<i32>>::into(PayloadType::AudioStop) {
                    done = true;
                }

                if addr_payload.payload.payload_type == <PayloadType as Into<i32>>::into(PayloadType::AudioCombined) && mode != "combined" {
                    if mode == "split-pcm16" {
                        let split_size = addr_payload.payload.audio.len() / 2;
                        let mut left = Vec::with_capacity(split_size as usize);
                        let mut right = Vec::with_capacity(split_size as usize);
                        for i in (0..split_size).step_by(4) {
                            let i = i as usize;
                            left.push(addr_payload.payload.audio[i]);
                            left.push(addr_payload.payload.audio[i+1]);
                            right.push(addr_payload.payload.audio[i+2]);
                            right.push(addr_payload.payload.audio[i+3]);
                        }
                        addr_payload.payload.audio_left = left;
                        addr_payload.payload.audio_right = right;
                        addr_payload.payload.payload_type = PayloadType::AudioSplit.into();
                        addr_payload.payload.audio.clear();
                    } else if mode == "split-mulaw" {
                        let split_size = addr_payload.payload.audio.len() / 2;
                        let mut left = Vec::with_capacity(split_size as usize);
                        let mut right = Vec::with_capacity(split_size as usize);
                        for i in (0..split_size).step_by(2) {
                            let i = i as usize;
                            left.push(addr_payload.payload.audio[i]);
                            right.push(addr_payload.payload.audio[i+1]);
                        }
                        addr_payload.payload.audio_left = left;
                        addr_payload.payload.audio_right = right;
                        addr_payload.payload.payload_type = PayloadType::AudioSplit.into();
                        addr_payload.payload.audio.clear();
                    }
                } else if addr_payload.payload.payload_type == <PayloadType as Into<i32>>::into(PayloadType::AudioStop) && address == addr_payload.address{
                    info!("stopping audio for uuid: {} to: {}", uuid, address);
                    done = true;
                }
                yield addr_payload.payload;
                if done {
                    info!("done streaming for uuid: {} to: {}", uuid, address);
                    break;
                }
            }
        };
        let request = tonic::Request::new(payload_stream);
        let _response = client.listen(request).await.unwrap();
    });

    let payload = AddressPayload {
        payload: Payload {
            uuid: uuid_clone,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            payload_type: PayloadType::AudioStart.into(),
            event_data: metadata.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    // send payload in channel log error
    if let Err(e) = channel.send(payload) {
        info!("failed to send to channel; error = {:?}", e);
        return Err(warp::reject());
    }

    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

// function to handle /dispatch_event endpoint
// accepts a json payload with uuid and event_data
// sends the event_data to the channel for the uuid
// returns ok
#[instrument(name = "dispatch_event", skip(channels))]
async fn dispatch_event_handler(
    body: HashMap<String, String>,
    channels: Arc<Mutex<UuidChannels>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = body.get("uuid").unwrap().clone();
    let event_data = body.get("event_data").unwrap().clone();
    let mut channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&uuid.clone()) {
        Some(channel) => channel.clone(),
        None => {
            info!("creating channel in http_server for uuid: {}", uuid);
            let (tx, _) = broadcast::channel(1000);
            channels.uuid_sender_map.insert(uuid.clone(), tx.clone());
            tx
        }
    };
    let payload = AddressPayload {
        payload: Payload {
            uuid: uuid.clone(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            event_data: event_data.clone(),
            payload_type: PayloadType::EventData.into(),
            ..Default::default()
        },
        ..Default::default()
    };

    if let Err(e) = channel.send(payload) {
        info!("failed to send to channel; error = {:?}", e);
        return Err(warp::reject());
    }
    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

#[instrument(name = "stop_cast", skip(channels))]
async fn stop_cast_handler(
    body: HashMap<String, String>,
    channels: Arc<Mutex<UuidChannels>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = body.get("uuid").unwrap().clone();
    let address = body.get("address").unwrap().clone();
    let metadata = body.get("metadata").unwrap_or(&"".to_string()).clone();
    let mut channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&uuid.clone()) {
        Some(channel) => channel.clone(),
        None => {
            info!("creating channel in http_server for uuid: {}", uuid);
            let (tx, _) = broadcast::channel(1000);
            channels.uuid_sender_map.insert(uuid.clone(), tx.clone());
            tx
        }
    };
    let payload = AddressPayload {
        payload: Payload {
            uuid: uuid.clone(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            payload_type: PayloadType::AudioStop.into(),
            event_data: metadata.clone(),
            ..Default::default()
        },
        address: address.clone(),
    };
    if let Err(e) = channel.send(payload) {
        info!("failed to send to channel; error = {:?}", e);
        return Err(warp::reject());
    }
    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}
