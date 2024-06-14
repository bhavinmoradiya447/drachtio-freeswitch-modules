use prometheus::{
    IntGaugeVec, IntCounterVec, Opts, Registry, Encoder
};
use warp::Reply;
use warp::Rejection;
use lazy_static::lazy_static;


lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    pub static ref ACTIVE_STREAMS: IntGaugeVec =
    IntGaugeVec::new(Opts::new("ACTIVE_STREAMS", "Active Streams"), &["client_address"]).expect("metric cannot be created");

    pub static ref TOTAL_SUCCESSFUL_STREAMS: IntCounterVec =
    IntCounterVec::new(Opts::new("TOTAL_SUCCESSFUL_STREAMS", "Total Successful Streams"), &["client_address"]).expect("metric cannot be created");

    pub static ref TOTAL_ERRORED_STREAMS: IntCounterVec =
    IntCounterVec::new(Opts::new("TOTAL_ERRORED_STREAMS", "Total Errored Streams"), &["client_address"]).expect("metric cannot be created");

}

pub async fn register_metrics() {
    REGISTRY
        .register(Box::new(ACTIVE_STREAMS.clone()))
        .expect("collector cannot be registered");

    REGISTRY
        .register(Box::new(TOTAL_SUCCESSFUL_STREAMS.clone()))
        .expect("collector cannot be registered");

    REGISTRY
        .register(Box::new(TOTAL_ERRORED_STREAMS.clone()))
        .expect("collector cannot be registered");
}


pub async fn metrics_handler() -> Result<impl Reply, Rejection> {
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}
