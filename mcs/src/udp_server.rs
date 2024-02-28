use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio::{
    net::UnixDatagram,
};
use tracing::{error, info};

use crate::mcs::PayloadType;
use crate::UuidChannels;
use crate::{AddressPayload, Payload};

pub async fn start_udp_server(
    channels: Arc<Mutex<UuidChannels>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = "/tmp/test-mcs-ds.sock"; // Replace with the actual path to your Unix domain socket
                                               // delete the socket file if it already exists
    tokio::fs::remove_file(socket_path).await.ok();
    // bind to the socket log on error
    let socket = UnixDatagram::bind(socket_path)?;
    tokio::spawn(async move {
        loop {
            let mut buffer = [0; 6432];
            let (size, _addr) = match socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => (size, addr),
                Err(e) => {
                    error!("failed to receive from socket; error = {:?}", e);
                    continue;
                }
            };
            // extract the uuid from first 16 bytes of the buffer
            let uuid = uuid::Uuid::from_slice(&buffer[..16]).unwrap().to_string();

            let mut channels = channels.lock().unwrap();
            {
                let mut done = false;
                // get channel from the map or log missing channel and continue
                let channel = match channels.uuid_sender_map.get(&uuid) {
                    Some(channel) => channel,
                    None => {
                        continue;
                    }
                };
                // send the buffer to the channel and log on error
                if let Err(e) = channel.send(parse_payload(buffer[..size].to_vec())) {
                    error!("failed to send to channel; error = {:?}", e);
                    // channels.uuid_sender_map.remove(&uuid);
                    // drop(channel);
                    done = true;
                }
                if size == 32 || done {
                    let channel = channels.uuid_sender_map.remove(&uuid);
                    drop(channel);
                    info!("removing channel for uuid: {}", uuid);
                }
            }
        }
    })
    .await?;
    Ok(())
}

fn parse_payload(buf: Vec<u8>) -> AddressPayload {
    let mut payload = Payload::default();
    payload.uuid = Uuid::from_slice(&buf[0..16]).unwrap().to_string();
    payload.timestamp = u64::from_ne_bytes(buf[20..28].try_into().unwrap());
    payload.audio = buf[32..].to_vec();
    if payload.audio.len() > 0 {
        payload.payload_type = PayloadType::AudioCombined.into();
    } else {
        payload.payload_type = PayloadType::AudioStop.into();
    }
    AddressPayload {
        payload,
        ..Default::default()
    }
}
