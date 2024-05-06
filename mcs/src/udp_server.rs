use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tokio::net::UnixDatagram;
use tracing::{error, info, trace};

use crate::mcs::DialogRequestPayloadType;
use crate::{UuidChannels, CONFIG};
use crate::{AddressPayload, DialogRequestPayload};

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
                let codec_sender = match channels.uuid_sender_map.get(&uuid) {
                    Some(channel) => channel,
                    None => {
                        continue;
                    }
                };
                let channel = &codec_sender.sender;
                // send the buffer to the channel and log on error
                if let Err(_e) = channel.send(
                    parse_payload(buffer[..size].to_vec(), codec_sender.codec.clone())) {
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

fn parse_payload(buf: Vec<u8>, codec: String) -> AddressPayload {
    let mut payload = DialogRequestPayload::default();
    payload.uuid = Uuid::from_slice(&buf[0..16]).unwrap().to_string();
    payload.timestamp = u64::from_ne_bytes(buf[20..28].try_into().unwrap());
    payload.audio = buf[32..].to_vec();
    let split_len = payload.audio.len() / 2;
    payload.audio_left = Vec::with_capacity(split_len);
    payload.audio_right = Vec::with_capacity(split_len);
    if payload.audio.len() > 0 {
        payload.payload_type = DialogRequestPayloadType::AudioCombined.into();
        if codec == "pcm16" {
            for i in (0..payload.audio.len()).step_by(4) {
                let i = i as usize;
                payload.audio_left.push(payload.audio[i]);
                payload.audio_left.push(payload.audio[i + 1]);
                payload.audio_right.push(payload.audio[i + 2]);
                payload.audio_right.push(payload.audio[i + 3]);
            }
        } else {
            for i in (0..payload.audio.len()).step_by(2) {
                let i = i as usize;
                payload.audio_left.push(payload.audio[i]);
                payload.audio_right.push(payload.audio[i + 1]);
            }
        }
    } else {
        payload.payload_type = DialogRequestPayloadType::AudioEnd.into();
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
        let payload = parse_payload(buf, "pcm16".to_string());
        assert_eq!(payload.payload.uuid, uuid.to_string());
        assert_eq!(payload.payload.audio.len(), 0);
    }
}
