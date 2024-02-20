use std::sync::{Arc, Mutex};

use tokio::{net::UnixDatagram, sync::broadcast::{self, Sender}};
use tracing::{info, error};

use crate::UuidChannels;

pub async fn start_udp_server(channels: Arc<Mutex<UuidChannels>>) -> Result<(), Box<dyn std::error::Error>>{
    let socket_path = "/tmp/test-mcs-ds.sock"; // Replace with the actual path to your Unix domain socket
    // delete the socket file if it already exists
    tokio::fs::remove_file(socket_path).await.ok();
    // bind to the socket log on error
    let socket = UnixDatagram::bind(socket_path)?;
    tokio::spawn(async move {
        loop {
            let mut buffer = [0; 1024];
            let (size, addr) = match socket.recv_from(&mut buffer).await {
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
                let channel = channels.uuid_sender_map.entry(uuid.clone()).or_insert_with(|| {
                    info!("creating channel for client in udp_server: {}", uuid);
                    create_channel(uuid.clone())
                });
                // send the buffer to the channel and log on error
                if let Err(e) = channel.send(buffer[..size].to_vec()) {
                    error!("failed to send to channel; error = {:?}", e);
                    return e;
                }
                if size == 32 {
                    channels.uuid_sender_map.remove(&uuid);
                }
            }
        }
    }).await?;
    Ok(())
}


fn create_channel(client: String) -> Sender<Vec<u8>> {
    info!("creating channel for client: {}", client);
    let (tx, _) = broadcast::channel(1000);
    tx
}