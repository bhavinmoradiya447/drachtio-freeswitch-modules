pub mod mcs {
    tonic::include_proto!("mcs");
}

mod http_server;
mod settings;
mod udp_server;
mod fs_tcp_client;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast::Sender;
use tracing::info;
use tracing_appender::rolling::daily;

use crate::http_server::start_http_server;
use crate::mcs::DialogRequestPayload;
use crate::udp_server::start_udp_server;
use lazy_static::lazy_static;
use crate::fs_tcp_client::start_fs_esl_client;

lazy_static! {
    static ref CONFIG: settings::Settings =
        settings::Settings::new().expect("config cannot be loaded");
}

#[derive(Debug, Default, Clone)]
struct AddressPayload {
    address: String,
    payload: DialogRequestPayload,
}

impl AddressPayload {
    fn new(uuid: String, payload_type: i32, address: String, event_data: String) -> Self {
        let payload = DialogRequestPayload {
            uuid,
            payload_type,
            event_data,
            ..Default::default()
        };
        AddressPayload { address, payload }
    }

    fn new_with_event_data(uuid: String, payload_type: i32, event_data: String) -> Self {
        let payload = DialogRequestPayload {
            uuid,
            payload_type,
            event_data,
            ..Default::default()
        };
        AddressPayload {
            payload,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default)]
struct UuidChannels {
    uuid_sender_map: HashMap<String, Sender<AddressPayload>>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_log();
    info!("starting mcs server");
    let channels = Arc::new(Mutex::new(UuidChannels::default()));
    let (event_publisher, event_receiver) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::try_join!(
        start_http_server(Arc::clone(&channels), event_publisher.clone()),
        start_udp_server(Arc::clone(&channels)),
        start_fs_esl_client(event_receiver, event_publisher.clone(), CONFIG.fs_esl_client.host.clone()),
    )?;
    Ok(())
}

fn init_log() {
    let logger = tracing_subscriber::fmt().json().flatten_event(true);
    if CONFIG.log.appender == "file" {
        let file_appender = daily(
            CONFIG.log.log_dir.clone(),
            CONFIG.log.log_prefix.clone());
        logger.with_writer(file_appender).init();
    } else {
        logger.init();
    }
}
