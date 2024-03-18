use std::{path::PathBuf, process::Child};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use lazy_static::lazy_static;

use reqwest::blocking;
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

mod grpc_server;

const SLEEP_DURATION_MILLIS: u64 = 20;
const SLEEP_DURATION_SECS: u64 = 1;
const CHUNK_SIZE: usize = 640;

lazy_static! {

static ref GLOBAL_MAP: Mutex<HashMap<String, i32>> = {

Mutex::new(HashMap::new())};

}
//#[test]
#[tokio::test]
async fn test() {
    // init tracing
    tracing_subscriber::fmt::init();

    let mut mcs_child = run_bin("mcs".to_string());
    let mut recorder_child = run_bin("recorder".to_string());

    tokio::spawn(async move {
        grpc_server::start_grpc_server().await.expect("TODO: panic message");
    });
    // wait for the mcs binary to start
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;


    let (tx, rx) = std::sync::mpsc::channel::<TcpStream>();
    let t0 = std::thread::spawn(|| {
        start_tcp_server(tx);
    });

    let t1 = std::thread::spawn(|| {
        test_ping();
    });

    let t2 = std::thread::spawn(|| {
        test_split_mulaw();
    });

    let t3 = std::thread::spawn(|| {
        test_mulaw();
    });

    let t4 = std::thread::spawn(|| {
        test_mulaw_segment();
    });

    let t5 = std::thread::spawn(|| {
        test_start_fail();
    });

    let t6 = std::thread::spawn(|| {
        test_stop_fail();
    });


    let t7 = std::thread::spawn(|| {
        test_mulaw_send_response_end();
        test_mulaw_dialog_end();
        test_mulaw_send_event();
        test_mulaw_send_audio();
    });


    if let Err(e) = t1.join() {
        error!("Failed on T1 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }
    if let Err(e) = t2.join() {
        error!("Failed on T2 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }

    if let Err(e) = t3.join() {
        error!("Failed on T3 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }

    if let Err(e) = t4.join() {
        error!("Failed on T4 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }

    if let Err(e) = t5.join() {
        error!("Failed on T5 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }

    if let Err(e) = t6.join() {
        error!("Failed on T6 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }

    if let Err(e) = t7.join() {
        error!("Failed on T7 {:?}",  e);
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        panic!("{:?}", e);
    }

    if t0.is_finished() {
        if let Err(e) = t0.join() {
            error!("Failed on T0 {:?}", e);
            mcs_child.kill().expect("failed to terminate mcs");
            recorder_child.kill().expect("failed to terminate recorder");
            panic!("{:?}", e);
        }
    } else {
        // terminate the mcs and recorder binary
        mcs_child.kill().expect("failed to terminate mcs");
        recorder_child.kill().expect("failed to terminate recorder");
        rx.recv().unwrap().shutdown(Shutdown::Both).unwrap();
    }

    let map = GLOBAL_MAP.lock().unwrap();

    assert_eq!(&3, map.get("start").unwrap());
    assert_eq!(&1, map.get("stop").unwrap());
    assert_eq!(&2, map.get("failed").unwrap());
    info!("Event value map {:?}", map);
}


fn start_tcp_server(tx: Sender<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:8022").unwrap();

    let (mut socket, _) = listener.accept().unwrap();
    tx.send(socket.try_clone().unwrap()).unwrap();
    socket.set_nodelay(true).unwrap();
    socket.write_all("Content-Type: auth/request".as_bytes()).unwrap();
    let mut buf = [0; 100];
    let size = socket.read(&mut buf).unwrap();
    if String::from_utf8(buf[0..size].to_owned()).unwrap().contains("auth Lcqzoc4e!zk3C3!#") {
        socket.write_all("Reply-Text: +OK accepted".as_bytes()).unwrap();
        loop {
            let mut buf = [0; 1021];
            let size = socket.read(&mut buf).unwrap();
            let event = String::from_utf8(buf[0..size].to_owned()).unwrap();
            info!("Got Command  {}", &event);
            let mut map = GLOBAL_MAP.lock().unwrap();
            if event.contains("mod_audio_cast::mcs::start") {
                let value = match map.remove("start") {
                    Some(v) => v,
                    _ => 0
                };
                map.insert("start".parse().unwrap(), value + 1);
            } else if event.contains("mod_audio_cast::mcs::stop") {
                let value = match map.remove("stop") {
                    Some(v) => v,
                    _ => 0
                };
                map.insert("stop".parse().unwrap(), value + 1);
            } else if event.contains("mod_audio_cast::mcs::failed") {
                let value = match map.remove("failed") {
                    Some(v) => v,
                    _ => 0
                };
                map.insert("failed".parse().unwrap(), value + 1);
            } else if event.contains("mod_audio_cast::mcs::event") {
                let value = match map.remove("event") {
                    Some(v) => v,
                    _ => 0
                };
                map.insert("event".parse().unwrap(), value + 1);
            }
            socket.write_all("Content-Type: command/reply\nReply-Text: +OK accepted".as_bytes()).unwrap();
        }
    } else {
        socket.write_all("-ERR Command not found!".as_bytes()).unwrap();
        assert!(false, "Unauthorized");
        socket.shutdown(Shutdown::Both).unwrap();
    }
}

fn test_ping() {
    info!("testing ping");
    let response = blocking::get("http://localhost:3030/ping").unwrap();
    assert!(response.status().is_success());
    info!("ping response status: {:?}", response.status());
}

fn test_split_mulaw() {
    info!("testing split-mulaw");
    let uuid = uuid::Uuid::new_v4();
    start_cast(uuid, "split".to_string(), "http://127.0.0.1:50051/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), false);
    validate_split_output(uuid);
    cleanup(uuid);
    info!("split-mulaw test passed");
}

fn test_mulaw() {
    info!("testing combined");
    let uuid = uuid::Uuid::new_v4();
    start_cast(uuid, "combined".to_string(), "http://127.0.0.1:50051/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), false);
    validate_output(uuid);
    cleanup(uuid);
    info!("combined test passed");
}

fn test_mulaw_dialog_end() {
    info!("testing dialog end");
    let uuid = Uuid::parse_str(grpc_server::UUID_FAILED_DIALOG).unwrap();
    start_cast(uuid, "combined".to_string(), "http://127.0.0.1:50052/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), false);
    info!("combined test passed");
}

fn test_mulaw_send_event() {
    info!("testing send event");
    let uuid = Uuid::parse_str(grpc_server::UUID_SEND_EVENT).unwrap();
    start_cast(uuid, "combined".to_string(), "http://127.0.0.1:50052/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), false);
    info!("combined test passed");
}

fn test_mulaw_send_audio() {
    info!("testing send audio");
    let uuid = Uuid::parse_str(grpc_server::UUID_SEND_AUDIO).unwrap();
    start_cast(uuid, "combined".to_string(), "http://127.0.0.1:50052/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), false);
    info!("combined test passed");
}

fn test_mulaw_send_response_end() {
    info!("testing response end");
    let uuid = uuid::Uuid::new_v4();
    start_cast(uuid, "combined".to_string(), "http://127.0.0.1:50052/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), false);
    info!("combined test passed");
}

fn test_mulaw_segment() {
    info!("testing mulaw-segment");
    let uuid = uuid::Uuid::new_v4();
    start_cast(uuid, "segment".to_string(), "http://127.0.0.1:50051/".parse().unwrap());
    stream_audio(uuid, "./resources/test-input-mulaw.raw".to_string(), true);
    validate_segement(uuid);
    cleanup(uuid);
    info!("mulaw-segment test passed");
}

fn test_start_fail() {
    info!("testing split-mulaw");
    let uuid = uuid::Uuid::new_v4();
    start_cast_failure(uuid, "split".to_string());
}

fn test_stop_fail() {
    info!("testing split-mulaw");
    let uuid = uuid::Uuid::new_v4();
    stop_cast_failure(uuid);
}

fn run_bin(cmd: String) -> Child {
    info!("running cmd: {:?}", cmd);
    let mut dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .to_path_buf();
    let mut path = dir.clone();
    if cmd == "mcs" {
        dir.push("mcs");
        if std::path::Path::new("mcs/target/release/mcs").exists() {
            path.push("mcs/target/release/mcs");
        } else {
            path.push("mcs/target/debug/mcs");
        }
    } else if cmd == "recorder" {
        dir.push("recorder");
        if std::path::Path::new("recorder/target/release/recorder").exists() {
            path.push("recorder/target/release/recorder");
        } else {
            path.push("recorder/target/debug/recorder");
        }
    }
    println!("running cmd: {:?} in dir {:?}", path, dir);
    let child = std::process::Command::new(path)
        .current_dir(dir)
        .spawn()
        .expect("failed to execute child");
    child
}

fn start_cast(uuid: Uuid, mode: String, grpc_address: String) {
    // send start_cast request
    let url = "http://127.0.0.1:3030/start_cast";
    let body = json!({
        "uuid": uuid.to_string(),
        "address": grpc_address,
        "mode": mode,
        "codec": "mulaw",
        "metadata": "test-metadata",
    });

    let client = reqwest::blocking::Client::new();
    let response = client.post(url).json(&body).send().unwrap();

    info!("start_cast response status: {:?}", response.status());
    // Check the response status
    assert!(response.status().is_success());
}

fn start_cast_failure(uuid: Uuid, mode: String) {
    // send start_cast request
    let url = "http://127.0.0.1:3030/start_cast";
    let body = json!({
        "uuid": uuid.to_string(),
        "address": "http://127.0.0.1:50054/",
        "mode": mode,
        "codec": "mulaw",
        "metadata": "test-metadata",
    });

    let client = reqwest::blocking::Client::new();
    let response = client.post(url).json(&body).send().unwrap();

    info!("start_cast response status: {:?}", response.status());
    // Check the response status
    assert!(response.status().is_success());
}

fn dispatch_event(uuid: Uuid, event: String) {
    // send dispatch_event request
    let url = "http://127.0.0.1:3030/dispatch_event";
    let body = json!({
        "uuid": uuid.to_string(),
        "event_data": event,
    });

    let client = reqwest::blocking::Client::new();
    let response = client.post(url).json(&body).send().unwrap();

    info!("dispatch_event response status: {:?}", response.status());
    // Check the response status
    assert!(response.status().is_success());
}

fn stop_cast(uuid: Uuid) {
    // send stop_cast request
    let url = "http://127.0.0.1:3030/stop_cast";
    let body = json!({
        "uuid": uuid.to_string(),
        "address": "http://127.0.0.1:50051/",
        "metadata": "test-metadata",
    });

    let client = reqwest::blocking::Client::new();
    let response = client.post(url).json(&body).send().unwrap();

    info!("stop_cast response status: {:?}", response.status());
    // Check the response status
    assert!(response.status().is_success());
}

fn stop_cast_failure(uuid: Uuid) {
    // send stop_cast request
    let url = "http://127.0.0.1:3030/stop_cast";
    let body = json!({
        "uuid": uuid.to_string(),
        "address": "http://127.0.0.1:50051/",
        "metadata": "test-metadata",
    });

    let client = reqwest::blocking::Client::new();
    let response = client.post(url).json(&body).send().unwrap();

    info!("stop_cast response status: {:?}", response.status());
    // Check the response status
    assert!(response.status().is_success());
}

fn create_payload(uuid: Uuid, seq: u32, len: u32, data: &[u8]) -> Vec<u8> {
    let timestamp = chrono::Utc::now().timestamp_millis().to_le_bytes();
    let mut payload = Vec::new();
    payload.extend_from_slice(uuid.as_bytes());
    payload.extend_from_slice(&seq.to_le_bytes());
    payload.extend_from_slice(&timestamp);
    payload.extend_from_slice(&len.to_le_bytes());
    payload.extend_from_slice(data);
    payload
}

fn create_socket(server_socket_path: &str) -> std::os::unix::net::UnixDatagram {
    let socket = std::os::unix::net::UnixDatagram::unbound().unwrap();
    socket.connect(server_socket_path).unwrap();
    socket
}

fn stream_audio(uuid: Uuid, file: String, segment: bool) {
    let input = std::fs::read(file).unwrap();
    let socket = create_socket("/tmp/mcs.sock");

    let mut seq: u32 = 0;
    for chunk in input.chunks(CHUNK_SIZE) {
        let payload = create_payload(uuid, seq, CHUNK_SIZE as u32, chunk);
        socket.send(&payload).unwrap();
        seq += 1;
        std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION_MILLIS));
        if seq == 10 {
            dispatch_event(uuid, "test-event".to_string());
        }
        if seq == 100 && segment {
            stop_cast(uuid);
            std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION_MILLIS));
        }
    }

    let payload = create_payload(uuid, seq, 0, &[]);
    socket.send(&payload).unwrap();

    info!(
        "Sent final payload with seq: {}, for uuid: {}",
        seq,
        uuid.to_string()
    );

    std::thread::sleep(std::time::Duration::from_secs(SLEEP_DURATION_SECS));
    drop(socket);
}

fn validate_output(uuid: Uuid) {
    // diff /tmp/rec-{uuid}.raw ./resources/test-input-mulaw.raw
    let output = std::process::Command::new("diff")
        .arg(format!("/tmp/rec-{}.raw", uuid))
        .arg("./resources/test-input-mulaw.raw")
        .output()
        .expect("failed to execute diff");

    assert!(output.status.success());
}

fn validate_segement(uuid: Uuid) {
    // diff /tmp/rec-{uuid}-0.raw ./resources/test-input-mulaw.raw
    let output = std::process::Command::new("diff")
        .arg(format!("/tmp/rec-{}.raw", uuid))
        .arg("./resources/output-segment-mulaw.raw")
        .output()
        .expect("failed to execute diff");

    info!("diff /tmp/rec-{uuid}.raw ./resources/output-segment-mulaw.raw: {:?}", output.status.success());
    assert!(output.status.success());
}


fn validate_split_output(uuid: Uuid) {
    // diff /tmp/rec-{uuid}-left.raw ./resources/test-input-mulaw.raw
    let output = std::process::Command::new("diff")
        .arg(format!("/tmp/rec-{}-left.raw", uuid))
        .arg("./resources/output-left-mulaw.raw")
        .output()
        .expect("failed to execute diff");

    assert!(output.status.success());

    // diff /tmp/rec-{uuid}-right.raw ./resources/test-input-mulaw.raw
    let output = std::process::Command::new("diff")
        .arg(format!("/tmp/rec-{}-right.raw", uuid))
        .arg("./resources/output-right-mulaw.raw")
        .output()
        .expect("failed to execute diff");

    assert!(output.status.success());
}

fn cleanup(uuid: Uuid) {
    // remove /tmp/rec-{uuid}.raw
    std::fs::remove_file(format!("/tmp/rec-{}.raw", uuid)).unwrap();
    std::fs::remove_file(format!("/tmp/rec-{}-left.raw", uuid)).unwrap();
    std::fs::remove_file(format!("/tmp/rec-{}-right.raw", uuid)).unwrap();
}
