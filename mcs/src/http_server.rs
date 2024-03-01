use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Uri};
use tonic::Status;
use tracing::{error, info, instrument};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::Config;
use warp::{
    hyper::{Response, StatusCode},
    path::{FullPath, Tail},
    Filter, Rejection, Reply,
};

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_client::MultiCastServiceClient;
use crate::mcs::PayloadType;
use crate::AddressPayload;
use crate::UuidChannels;

const GRPC_CONNECTIONS: usize = 20;

#[derive(Debug, Default, Clone)]
struct TokenInterceptor;

impl tonic::service::Interceptor for TokenInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        let token = std::fs::read_to_string("/usr/local/token.txt").ok();
        if let Some(token) = token {
            let bearer_token = format!("Bearer {}", token);
            request.metadata_mut().insert("authorization", bearer_token.parse().unwrap());
        }
        Ok(request) 
    }
}


#[derive(Debug, Default)]
struct AddressClients {
    clients: HashMap<String, MultiCastServiceClient<InterceptedService<Channel, TokenInterceptor>>>,
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

    // swagger ui
    let config = Arc::new(Config::from("/api-doc.json"));

    #[derive(OpenApi)]
    #[openapi(
        paths(
            crate::http_server::start_cast_handler, 
            crate::http_server::dispatch_event_handler, 
            crate::http_server::stop_cast_handler,
            crate::http_server::ping,
        ),
        components(
            schemas(crate::http_server::StartCastRequest, 
                crate::http_server::DispatchEventRequest, 
                crate::http_server::StopCastRequest,
            ),
        ),
        tags(
            (name = "MCS API", description = "Multi Cast Streamer API")
        )
    )]
    struct ApiDoc;

    let api_doc = warp::path("api-doc.json")
        .and(warp::get())
        .map(|| warp::reply::json(&ApiDoc::openapi()));

    let swagger_ui = warp::path("swagger-ui")
        .and(warp::get())
        .and(warp::path::full())
        .and(warp::path::tail())
        .and(warp::any().map(move || config.clone()))
        .and_then(serve_swagger);

    let routes = start_cast
        .or(filters::get_ping())
        .or(api_doc)
        .or(swagger_ui)
        .or(stop_cast)
        .or(dispatch_event);

    info!("starting http server");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

async fn serve_swagger(
    full_path: FullPath,
    tail: Tail,
    config: Arc<Config<'static>>,
) -> Result<Box<dyn Reply + 'static>, Rejection> {
    if full_path.as_str() == "/swagger-ui" {
        return Ok(Box::new(warp::redirect::found(Uri::from_static(
            "/swagger-ui/",
        ))));
    }

    let path = tail.as_str();
    match utoipa_swagger_ui::serve(path, config) {
        Ok(file) => {
            if let Some(file) = file {
                Ok(Box::new(
                    Response::builder()
                        .header("Content-Type", file.content_type)
                        .body(file.bytes),
                ))
            } else {
                Ok(Box::new(StatusCode::NOT_FOUND))
            }
        }
        Err(error) => Ok(Box::new(
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(error.to_string()),
        )),
    }
}

#[derive(Debug, serde::Deserialize, ToSchema)]
struct StartCastRequest {
    uuid: String,
    address: String,
    mode: Option<String>,
    metadata: Option<String>,
}

// create fn to get address uri from input address string
// read contents of /usr/local/token.txt and set token as
// authorization header if it exists
fn get_address_uri(address: String) -> Uri {
    let address_uri = Uri::try_from(&address).unwrap();
    let token = std::fs::read_to_string("/usr/local/token.txt").ok();
    if let Some(token) = token {
        let mut headers = http::HeaderMap::new();
        let bearer_token = format!("Bearer {}", token);
        headers.insert("authorization", bearer_token.parse().unwrap());
        address_uri
    } else {
        address_uri
    }
}

#[utoipa::path(post, path = "/start_cast", request_body = StartCastRequest)]
#[instrument(name = "start_cast", skip(channels, address_client))]
async fn start_cast_handler(
    // body: HashMap<String, String>,
    request: StartCastRequest,
    channels: Arc<Mutex<UuidChannels>>,
    address_client: Arc<Mutex<AddressClients>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let count = COUNTER.fetch_add(1, SeqCst);
    let uuid = request.uuid.clone();
    let uuid_clone = uuid.clone();
    let address = request.address.clone();
    let mode = request.mode.unwrap_or("combined".to_string()).clone();
    let metadata = request.metadata.unwrap_or("".to_string()).clone();
    let group = (count / GRPC_CONNECTIONS) % GRPC_CONNECTIONS;
    let address_key = format!("{}-{}", address, group);
    let address_uri = get_address_uri(address.clone());

    let mut address_client = address_client.lock().unwrap();
    let mut client = match address_client.clients.get(&address_key) {
        Some(client) => client.clone(),
        None => {
            info!(
                "creating client for address: {} in group: {}",
                address, group
            );
            let grpc_channel = Channel::builder(address_uri).connect_lazy();
            let grpc_client = MultiCastServiceClient::with_interceptor(
                grpc_channel, TokenInterceptor);
            address_client
                .clients
                .insert(address_key, grpc_client.clone());
            grpc_client
        }
    };

    let mut channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&uuid) {
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
        info!("init payload stream for uuid: {} to: {}", uuid, address);
        let payload_stream = async_stream::stream! {
            while let Ok(addr_payload) = receiver.recv().await {
                let mut payload = addr_payload.payload;
                let payload_type = payload.payload_type;
                if payload_type == eval(&PayloadType::AudioCombined) && mode != "combined" {
                    let (left, right) = handle_split(&payload.audio, mode.clone());
                    payload.audio_left = left;
                    payload.audio_right = right;
                    payload.payload_type = PayloadType::AudioSplit.into();
                    payload.audio.clear();
                }
                yield payload;
                if payload_type == eval(&PayloadType::AudioEnd)
                    || (payload_type == eval(&PayloadType::AudioStop)
                        && address == addr_payload.address) {
                    info!("done streaming for uuid: {} to: {}", uuid, address);
                    break;
                }
            }
        };
        let request = tonic::Request::new(payload_stream);
        let _response = client.listen(request).await.unwrap();
    });

    if let Err(e) = channel.send(AddressPayload::new_with_event_data(
        uuid_clone,
        PayloadType::AudioStart.into(),
        metadata,
    )) {
        info!("failed to send to channel; error = {:?}", e);
        return Err(warp::reject());
    }

    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

fn eval(payload_type: &PayloadType) -> i32 {
    *payload_type as i32
}

#[instrument(name = "handle_split")]
fn handle_split(audio: &Vec<u8>, mode: String) -> (Vec<u8>, Vec<u8>) {
    let split_size = audio.len() / 2;
    let mut left = Vec::with_capacity(split_size as usize);
    let mut right = Vec::with_capacity(split_size as usize);
    if mode == "split-pcm16" {
        for i in (0..split_size).step_by(4) {
            let i = i as usize;
            left.push(audio[i]);
            left.push(audio[i + 1]);
            right.push(audio[i + 2]);
            right.push(audio[i + 3]);
        }
    } else if mode == "split-mulaw" {
        for i in (0..split_size).step_by(2) {
            let i = i as usize;
            left.push(audio[i]);
            right.push(audio[i + 1]);
        }
    }
    (left, right)
}

#[derive(Debug, serde::Deserialize, ToSchema)]
struct DispatchEventRequest {
    uuid: String,
    event_data: String,
}

#[utoipa::path(post, path = "/dispatch_event", request_body = DispatchEventRequest)]
#[instrument(name = "dispatch_event", skip(channels))]
async fn dispatch_event_handler(
    request: DispatchEventRequest,
    channels: Arc<Mutex<UuidChannels>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&request.uuid) {
        Some(channel) => channel.clone(),
        None => {
            error!("channel not found for uuid: {}", request.uuid);
            return Err(warp::reject());
        }
    };

    if let Err(e) = channel.send(AddressPayload::new_with_event_data(
        request.uuid,
        PayloadType::EventData.into(),
        request.event_data,
    )) {
        info!("failed to send to channel; error = {:?}", e);
        return Err(warp::reject());
    }
    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

#[derive(Debug, serde::Deserialize, ToSchema)]
struct StopCastRequest {
    uuid: String,
    address: String,
    metadata: Option<String>,
}

#[utoipa::path(post, path = "/stop_cast", request_body = StopCastRequest)]
#[instrument(name = "stop_cast", skip(channels))]
async fn stop_cast_handler(
    request: StopCastRequest,
    channels: Arc<Mutex<UuidChannels>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&request.uuid) {
        Some(channel) => channel.clone(),
        None => {
            error!("channel not found for uuid: {}", request.uuid);
            // throw error if channel does not exist
            return Err(warp::reject());
        }
    };
    if let Err(e) = channel.send(AddressPayload::new(
        request.uuid,
        PayloadType::AudioStop.into(),
        request.address,
        request.metadata.unwrap_or("".to_string()),
    )) {
        info!("failed to send to channel; error = {:?}", e);
        return Err(warp::reject());
    }
    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

#[utoipa::path(get, path = "/ping")]
#[instrument(name = "ping")]
pub async fn ping() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&"pong"))
}
mod filters {
    use warp::Filter;
    use crate::http_server::ping;

    pub fn get_ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("ping")
            .and(warp::get())
            .and_then(ping)
    }
}

// unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_split() {
        let audio = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let (left, right) = handle_split(&audio, "split-pcm16".to_string());
        assert_eq!(left, vec![1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[tokio::test]
    async fn test_ping () {
        let api = filters::get_ping();
        let resp = warp::test::request()
            .method("GET")
            .path("/ping")
            .reply(&api)
            .await;
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.body(), "\"pong\"");
    }
}
