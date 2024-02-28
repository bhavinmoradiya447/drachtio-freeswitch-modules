pub mod mcs {
    tonic::include_proto!("mcs");
}

mod http_server;
mod udp_server;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast::Sender;
use tracing::info;

use crate::http_server::start_http_server;
use crate::mcs::Payload;
use crate::udp_server::start_udp_server;

#[derive(Debug, Default, Clone)]
struct AddressPayload {
    address: String,
    payload: Payload,
}

#[derive(Debug, Default)]
struct UuidChannels {
    uuid_sender_map: HashMap<String, Sender<AddressPayload>>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("starting mcs-ds server");
    let channels = Arc::new(Mutex::new(UuidChannels::default()));
    tokio::try_join!(
        start_http_server(Arc::clone(&channels)),
        start_udp_server(Arc::clone(&channels))
    )?;
    Ok(())
}
