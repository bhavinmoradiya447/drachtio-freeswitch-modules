use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use tonic::transport::{Channel, Uri};
use tracing::{info, instrument};
use uuid::Uuid;
use warp::Filter;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_client::MultiCastServiceClient;
use crate::mcs::Payload;
use crate::UuidChannels;

const GRPC_CONNECTIONS: i32 = 20;

#[derive(Debug, Default)]
struct AddressClients {
    clients: HashMap<String, MultiCastServiceClient<Channel>>,
}

pub async fn start_http_server(
    uuid_channels: Arc<Mutex<UuidChannels>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let with_uuid_channel = warp::any().map(move || Arc::clone(&uuid_channels));

    let address_client = Arc::new(Mutex::new(AddressClients::default()));
    let with_address_client = warp::any().map(move || Arc::clone(&address_client));

    let counter = Arc::new(Mutex::new(0));
    let with_counter = warp::any().map(move || {
        let mut counter = counter.lock().unwrap();
        *counter += 1;
        *counter
    });

    let start_cast = warp::path!("start_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel.clone())
        .and(with_address_client.clone())
        .and(with_counter.clone())
        .and_then(start_cast_handler);

    info!("starting http server");
    warp::serve(start_cast).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

#[instrument(name = "start_cast", skip(channels, address_client))]
async fn start_cast_handler(
    body: HashMap<String, String>,
    channels: Arc<Mutex<UuidChannels>>,
    address_client: Arc<Mutex<AddressClients>>,
    counter: i32,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = body.get("uuid").unwrap().clone();
    let address = body.get("address").unwrap().clone();
    let group = counter % GRPC_CONNECTIONS;
    let address_key = format!("{}-{}", address, group);
    let address_uri = Uri::try_from(&address).unwrap();
    let client1 = MultiCastServiceClient::connect(address_uri).await.unwrap();

    let mut address_client = address_client.lock().unwrap();
    let mut client = match address_client.clients.get(&address_key) {
        Some(client) => client.clone(),
        None => {
            address_client.clients.insert(address_key, client1.clone());
            client1
        }
    };

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

    let mut receiver = channel.subscribe();
    tokio::spawn(async move {
        info!(
            "initialising payload stream for uuid: {} to: {}",
            uuid, address
        );
        let payload_stream = async_stream::stream! {
            while let Ok(chunk) = receiver.recv().await {
                let len = chunk.len();
                yield parse_payload(chunk);
                if (len as u32) == 32 {
                    break;
                }
            }
        };
        let request = tonic::Request::new(payload_stream);
        let _response = client.listen(request).await.unwrap();
        info!("done streaming for uuid: {} to: {}", uuid, address);
    });

    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

fn parse_payload(buf: Vec<u8>) -> Payload {
    let mut payload = Payload::default();
    payload.uuid = Uuid::from_slice(&buf[0..16]).unwrap().to_string();
    payload.seq = u32::from_ne_bytes(buf[16..20].try_into().unwrap());
    payload.timestamp = u64::from_ne_bytes(buf[20..28].try_into().unwrap());
    payload.size = u32::from_ne_bytes(buf[28..32].try_into().unwrap());
    payload.audio = buf[32..].to_vec();
    payload
}
