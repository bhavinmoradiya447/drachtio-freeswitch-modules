use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio::net::UnixDatagram;
use tracing::{error, info, trace};

use crate::mcs::PayloadType;
use crate::{UuidChannels, CONFIG};
use crate::{AddressPayload, Payload};

pub async fn start_udp_server(
    channels: Arc<Mutex<UuidChannels>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // get socket path from the config
    let socket_path = CONFIG.udp_server.socket_file.clone();
    // delete the socket file if it already exists
    tokio::fs::remove_file(socket_path.clone()).await.ok();
    info!("binding to socket: {}", socket_path);
    // bind to the socket log on error
    let socket = UnixDatagram::bind(socket_path)?;
    tokio::spawn(async move {
        info!("started udp server");
        loop {
            let mut buffer = [0; 6432];
            let (size, _addr) = match socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => (size, addr),
                Err(e) => {
                    error!("failed to receive from socket; error = {:?}", e);
                    continue;
                }
            };
            trace!("received {} bytes", size);
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
                if let Err(_e) = channel.send(parse_payload(buffer[..size].to_vec())) {
                    error!("failed to send to channel; uuid = {}", uuid);
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

// unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_payload() {
        let mut buf = Vec::new();
        let uuid = Uuid::new_v4();
        buf.extend_from_slice(uuid.as_bytes());
        let seq: u32 = 0;
        buf.extend_from_slice(&seq.to_le_bytes());
        let timestamp = chrono::Utc::now().timestamp_millis().to_le_bytes();
        buf.extend_from_slice(&timestamp);
        let len: u32 = 0;
        buf.extend_from_slice(&len.to_le_bytes());
        let payload = parse_payload(buf);
        assert_eq!(payload.payload.uuid, uuid.to_string());
        assert_eq!(payload.payload.audio.len(), 0);
    }
}
