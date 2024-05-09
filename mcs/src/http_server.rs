use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedSender;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Uri};
use tonic::Status;
use tracing::{error, info, instrument};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::Config;
use warp::{
    hyper::{Response, StatusCode},
    path::{FullPath, Tail},
    Filter, Rejection, Reply,
};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::thread::sleep;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::ReusableBoxFuture;
use tonic::codegen::tokio_stream::{Stream, StreamExt};
use warp::header::value;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::media_cast_service_client::MediaCastServiceClient;
use crate::mcs::DialogRequestPayloadType;
use crate::mcs::DialogRequestPayload;
use crate::{AddressPayload, CodecSender};
use crate::mcs::DialogResponsePayload;
use crate::mcs::DialogResponsePayloadType;
use crate::UuidChannels;
use crate::CONFIG;
use crate::db_client::{CallDetails, DbClient};
use crate::fs_tcp_client::{get_event_command, get_start_failed_event_command, get_start_success_event_command, get_stop_failed_event_command, get_stop_success_event_command};

#[derive(Debug, Default, Clone)]
struct TokenInterceptor;

impl tonic::service::Interceptor for TokenInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        let token = std::fs::read_to_string(CONFIG.http_server.token_file.clone()).ok();
        if let Some(token) = token {
            let bearer_token = format!("Bearer {}", token);
            request.metadata_mut().insert("authorization", bearer_token.parse().unwrap());
        }
        Ok(request)
    }
}


#[derive(Debug, Default)]
struct AddressClients {
    clients: HashMap<String, MediaCastServiceClient<InterceptedService<Channel, TokenInterceptor>>>,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub async fn start_http_server(
    uuid_channels: Arc<Mutex<UuidChannels>>,
    event_sender: UnboundedSender<String>,
    db_client: Arc<DbClient>,
) -> Result<(), Box<dyn std::error::Error>> {
    let uuid_channels_clone = uuid_channels.clone();
    let event_sender_clone = event_sender.clone();
    let db_client_clone = db_client.clone();


    let with_uuid_channel = warp::any().map(move || Arc::clone(&uuid_channels));
    let with_event_sender = warp::any().map(move || event_sender.clone());
    let with_db_client = warp::any().map(move || db_client.clone());

    let address_client = Arc::new(Mutex::new(AddressClients::default()));
    let address_client_clone = address_client.clone();
    let with_address_client = warp::any().map(move || Arc::clone(&address_client));


    let call_details = db_client_clone.select_all();

    for call_detail in call_details.iter() {
        let retry = Arc::new(Mutex::new(Retry { retry_count: 0 }));
        start_cast(uuid_channels_clone.clone(), address_client_clone.clone(), event_sender_clone.clone(), db_client_clone.clone(),
                   call_detail.call_leg_id.clone(), call_detail.client_address.clone(), call_detail.codec.clone(),
                   call_detail.mode.clone(), call_detail.metadata.clone(), false, retry);
    }

    let start_cast = warp::path!("start_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel.clone())
        .and(with_address_client.clone())
        .and(with_event_sender.clone())
        .and(with_db_client.clone())
        .and_then(start_cast_handler);

    let stop_cast = warp::path!("stop_cast")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel.clone())
        .and(with_event_sender.clone())
        .and_then(stop_cast_handler);

    let stop_all = warp::path!("stop_all")
        .and(warp::post())
        .and(with_uuid_channel.clone())
        .and(with_event_sender.clone())
        .and_then(stop_all_handler);

    let dispatch_event = warp::path!("dispatch_event")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_uuid_channel)
        .and(with_event_sender.clone())
        .and_then(dispatch_event_handler);

    let ping = warp::path!("ping")
        .and(warp::get())
        .and_then(ping_handler);

    let routes = init_open_api()
        .or(init_swagger_ui())
        .or(ping)
        .or(start_cast)
        .or(stop_cast)
        .or(stop_all)
        .or(dispatch_event);

    info!("starting http server");
    warp::serve(routes).run(([127, 0, 0, 1], CONFIG.http_server.port)).await;
    Ok(())
}

fn init_open_api() -> impl Filter<Extract=(impl Reply, ), Error=warp::Rejection> + Clone {
    #[derive(OpenApi)]
    #[openapi(
    paths(
    crate::http_server::start_cast_handler,
    crate::http_server::dispatch_event_handler,
    crate::http_server::stop_cast_handler,
    crate::http_server::stop_all_handler,
    crate::http_server::ping_handler,
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
    api_doc
}

fn init_swagger_ui() -> impl Filter<Extract=(impl Reply, ), Error=warp::Rejection> + Clone {
    let config = Arc::new(Config::from("/api-doc.json"));
    let swagger_ui = warp::path("swagger-ui")
        .and(warp::get())
        .and(warp::path::full())
        .and(warp::path::tail())
        .and(warp::any().map(move || config.clone()))
        .and_then(serve_swagger);
    swagger_ui
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
    codec: Option<String>,
    mode: Option<String>,
    metadata: Option<String>,
}

#[utoipa::path(post, path = "/start_cast", request_body = StartCastRequest)]
#[instrument(name = "start_cast", skip(channels, address_client, db_client))]
async fn start_cast_handler(
    // body: HashMap<String, String>,
    request: StartCastRequest,
    channels: Arc<Mutex<UuidChannels>>,
    address_client: Arc<Mutex<AddressClients>>,
    event_sender: UnboundedSender<String>,
    db_client: Arc<DbClient>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = request.uuid.clone();
    let address = request.address.clone();
    let codec = request.codec.unwrap_or("mulaw".to_string());
    let mode = request.mode.unwrap_or("combined".to_string());
    let metadata = request.metadata.unwrap_or("".to_string()).clone();
    let retry = Arc::new(Mutex::new(Retry { retry_count: 0 }));
    start_cast(channels, address_client, event_sender, db_client, uuid, address, codec, mode, metadata, true, retry);
    info!("returning ok");
    Ok(warp::reply::json(&"ok"))
}

fn start_cast(channels: Arc<Mutex<UuidChannels>>, address_client: Arc<Mutex<AddressClients>>,
              event_sender: UnboundedSender<String>, db_client: Arc<DbClient>, uuid: String,
              address: String, codec: String, mode: String, metadata: String, insert_to_db: bool, retry: Arc<Mutex<Retry>>) {
    let count = COUNTER.fetch_add(1, SeqCst);
    let mode_clone = mode.clone();
    let uuid_clone = uuid.clone();
    let address_clone = address.clone();
    let group = (count / CONFIG.http_server.grpc_connection_pool) % CONFIG.http_server.grpc_connection_pool;
    let address_key = format!("{}-{}", address, group);
    let address_uri = Uri::try_from(&address).unwrap();
    let address_client_clone = address_client.clone();

    let mut address_client = address_client.lock().unwrap();
    let mut client = match address_client.clients.get(&address_key) {
        Some(client) => client.clone(),
        None => {
            info!(
                "creating client for address: {} in group: {}",
                address, group
            );
            let grpc_channel = if Path::new(CONFIG.http_server.tls_cert_file.as_str()).exists() {
                let pem = std::fs::read_to_string(CONFIG.http_server.tls_cert_file.as_str()).unwrap();
                let ca = Certificate::from_pem(pem);

                let tls = ClientTlsConfig::new()
                    .ca_certificate(ca);
                Channel::builder(address_uri).tls_config(tls).unwrap().connect_lazy()
            } else {
                Channel::builder(address_uri).connect_lazy()
            };
            let grpc_client = MediaCastServiceClient::with_interceptor(
                grpc_channel, TokenInterceptor);
            address_client
                .clients
                .insert(address_key, grpc_client.clone());
            grpc_client
        }
    };
    let channels_clone = channels.clone();
    let mut channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&uuid) {
        Some(channel) => channel.clone(),
        None => {
            info!("creating channel in http_server for uuid: {}", uuid);
            let (tx, _) = broadcast::channel(1000);
            // create CodecSender and insert into map
            let codec_sender = CodecSender {
                codec: codec.clone(),
                sender: tx.clone(),
            };
            channels.uuid_sender_map.insert(uuid.clone(), codec_sender.clone());
            codec_sender
        }
    }.sender.clone();
    let metadata_clone = metadata.clone();
    let receiver = channel.subscribe();
    let retry_clone = retry.clone().lock().unwrap();
    retry_clone.retry_count = retry_clone.retry_count + 1;
    let mut retry_stream = CastStreamWithRetry::new(receiver,
                                                    channels_clone.clone(),
                                                    address_client_clone.clone(),
                                                    event_sender.clone(),
                                                    db_client.clone(),
                                                    uuid_clone.clone(),
                                                    address_clone.clone(),
                                                    codec.clone(),
                                                    mode_clone.clone(),
                                                    metadata_clone.clone(),
                                                    retry.clone(),
    );

    let db_client_clone = db_client.clone();


    tokio::spawn(async move {
        info!("init payload stream for uuid: {} to: {}", uuid, address);
        let address_clone = address.clone();
        let uuid_clone = uuid.clone();
        let db_client_1 = db_client_clone.clone();
        let db_client_2 = db_client_clone.clone();
        //  let mut retry_stream_clone = retry_stream.clone();
        let mut retry_clone_1 = retry.clone();
        //let mut retry_clone_2 = retry.clone().lock().unwrap();


        let payload_stream = async_stream::stream! {
            while let Some(mut addr_payload_result) = retry_stream.next().await {
                match addr_payload_result {
                    Ok(mut addr_payload) => {
                        let payload_type = addr_payload.payload.payload_type;
                        process_payload(&mut addr_payload.payload, mode.clone());
                        if payload_type == eval(&DialogRequestPayloadType::AudioStart) && address.clone() != addr_payload.address {
                            continue;
                        }
                        yield addr_payload.payload;
                        if payload_type == eval(&DialogRequestPayloadType::AudioEnd)
                            || (payload_type == eval(&DialogRequestPayloadType::AudioStop) && address.clone() == addr_payload.address) {
                            info!("done streaming for uuid: {} to: {}", uuid.clone(), address.clone());
                            retry_clone_1.clone().lock().unwrap().retry_count = -1;
                            if payload_type == eval(&DialogRequestPayloadType::AudioEnd) {
                                let file_path = format!("/tmp/{}", uuid.clone());
                                if Path::new(file_path.as_str()).exists() {
                                 std::fs::remove_dir_all(file_path).expect("Failed to remove Directory");
                                }
                                db_client_1.delete_by_call_leg_and_client_address(uuid.clone(), address.clone());
                                let mut channels_ = channels_clone.lock().unwrap();
                                channels_.uuid_sender_map.remove(uuid.as_str());
                            }
                            break;
                        }
                    }
                    Err(e) => {error!("Gor Error on receiver Stream {:?}", e)}
                }
            }
        };
        let request = tonic::Request::new(payload_stream);
        let event_sender1 = event_sender.clone();
        match client.dialog(request).await {
            Ok(response) => {
                retry_clone_1.clone().lock().unwrap().retry_count = 1;
                tokio::spawn(async move {
                    let mut is_first_message = true;

                    let mut response = response.into_inner();
                    while let Some(payload) = response.message().await.unwrap() {
                        if is_first_message && payload.payload_type == eval1(&DialogResponsePayloadType::DialogEnd) {
                            event_sender1.send(get_start_failed_event_command(uuid_clone.as_str(),
                                                                              address_clone.as_str(),
                                                                              payload.data.as_str(),
                                                                              "subscriber-error"))
                                .expect("Failed to send start client error");
                            retry_clone_1.clone().lock().unwrap().retry_count = -11;
                        } else if is_first_message {
                            let mut data = metadata.as_str();
                            if payload.payload_type == eval1(&DialogResponsePayloadType::DialogStart) {
                                data = payload.data.as_str();
                            }
                            event_sender1.send(get_start_success_event_command(uuid_clone.as_str(),
                                                                               address_clone.as_str(),
                                                                               data))
                                .expect("Failed to send start success event");
                            if insert_to_db {
                                db_client_2.insert(CallDetails {
                                    call_leg_id: uuid_clone.clone(),
                                    client_address: address_clone.clone(),
                                    codec: codec.clone(),
                                    mode: mode_clone.clone(),
                                    metadata: metadata.clone(),
                                });
                            }
                        }
                        is_first_message = false;
                        if payload.payload_type == eval1(&DialogResponsePayloadType::ResponseEnd) {
                            break;
                        }
                        process_response_payload(uuid_clone.as_str(), address_clone.as_str(), &payload, event_sender1.clone());
                    }
                });
            }
            Err(e) => {
                error!("Error connecting client {} , {}", address_clone.clone(), e.message());
                if retry_clone_1.clone().lock().unwrap().retry_count == 3 {
                    event_sender.send(get_start_failed_event_command(uuid_clone.as_str(),
                                                                     address_clone.as_str(),
                                                                     metadata.as_str(),
                                                                     "connection-failed"))
                        .expect("Failed to send start failed event");
                }
                //drop(retry_stream);
            }
        };
    });

    if let Err(e) = channel.send(AddressPayload::new(
        uuid_clone.clone(),
        DialogRequestPayloadType::AudioStart.into(),
        address_clone.clone(),
        metadata_clone,
    )) {
        info!("failed to send to channel; error = {:?}", e);
        //return Err(warp::reject());
    }
}

fn process_response_payload(uuid: &str, address: &str, payload: &DialogResponsePayload, event_sender: UnboundedSender<String>) {
    // open file /tmp/uuid/{payload.uuid}.wav in append mode
    // write payload.audio to file
    // close file
    // log error on failure

    if payload.payload_type == eval1(&DialogResponsePayloadType::Event) {
        event_sender.send(get_event_command(uuid, address, "subscriber-event", payload.data.as_str()))
            .expect("Failed to send client event");
    } else if payload.payload_type == eval1(&DialogResponsePayloadType::AudioChunk) ||
        payload.payload_type == eval1(&DialogResponsePayloadType::EndOfAudio) {
        let dir = format!("/tmp/{}", uuid);
        if !Path::new(dir.as_str()).exists() {
            fs::create_dir(dir).unwrap();
        }
        let file_path = format!("/tmp/{}/{}.wav", uuid, payload.data);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path.clone())
            .unwrap();
        if let Err(e) = file.write_all(&payload.audio) {
            error!("failed to write to file; error = {:?}", e);
        }
        if let Err(e) = file.sync_all() {
            error!("failed to sync file; error = {:?}", e);
        }
        if payload.payload_type == eval1(&DialogResponsePayloadType::EndOfAudio) {
            if let Err(e) = file.flush() {
                error!("failed to flush file; error = {:?}", e);
            }
            let payload = format!("{{\"file_path\":\"{}\"}}", file_path);
            event_sender.send(get_event_command(uuid, address, "subscriber-playback", payload.as_str()))
                .expect("Failed to send client event");
        }
    }
}

fn process_payload(payload: &mut DialogRequestPayload, mode: String) {
    if payload.payload_type == eval(&DialogRequestPayloadType::AudioCombined) {
        if mode == "split" {
            payload.audio.clear();
        } else {
            payload.audio_left.clear();
            payload.audio_right.clear();
        }
    }
}

fn eval(payload_type: &DialogRequestPayloadType) -> i32 {
    *payload_type as i32
}

fn eval1(payload_type: &DialogResponsePayloadType) -> i32 {
    *payload_type as i32
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
    event_sender: UnboundedSender<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&request.uuid) {
        Some(channel) => channel.clone(),
        None => {
            error!("channel not found for uuid: {}", request.uuid);
            return Err(warp::reject());
        }
    };

    if let Err(e) = channel.sender.send(AddressPayload::new_with_event_data(
        request.uuid,
        DialogRequestPayloadType::EventData.into(),
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
    event_sender: UnboundedSender<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let uuid = request.uuid;
    let address = request.address;
    let metadata = request.metadata.unwrap_or("".to_string());
    let channels = channels.lock().unwrap();
    let channel = match channels.uuid_sender_map.get(&uuid) {
        Some(channel) => Some(channel.clone()),
        None => {
            error!("channel not found for uuid: {}", uuid);
            // throw error if channel does not exist
            //return Err(warp::reject());
            event_sender.send(get_stop_failed_event_command(uuid.clone().as_str(),
                                                            address.clone().as_str(),
                                                            metadata.clone().as_str(),
                                                            "channel-not-exist"))
                .expect("Failed to send stop failure");
            None
        }
    };
    if channel.is_some() {
        if let Err(e) = channel.unwrap().sender.send(AddressPayload::new(
            uuid.clone(),
            DialogRequestPayloadType::AudioStop.into(),
            address.clone(),
            metadata.clone(),
        )) {
            info!("failed to send to channel; error = {:?}", e);
            //return Err(warp::reject());
            event_sender.send(get_stop_failed_event_command(uuid.clone().as_str(),
                                                            address.clone().as_str(),
                                                            metadata.clone().as_str(),
                                                            "failed-to-send"))
                .expect("Failed to send stop failure");
        }
        info!("returning ok");
        event_sender.send(get_stop_success_event_command(uuid.clone().as_str(),
                                                         address.clone().as_str(),
                                                         metadata.clone().as_str()))
            .expect("Failed to send stop success");
    }
    Ok(warp::reply::json(&"ok"))
}

#[utoipa::path(post, path = "/stop_all")]
#[instrument(name = "stop_all", skip(channels))]
async fn stop_all_handler(
    channels: Arc<Mutex<UuidChannels>>,
    event_sender: UnboundedSender<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut channels = channels.lock().unwrap();

    for (uuid, value) in channels.uuid_sender_map.drain() {
        let payload = DialogRequestPayload {
            uuid: uuid.clone(),
            payload_type: DialogRequestPayloadType::AudioEnd.into(),
            ..Default::default()
        };

        if let Err(e) = value.sender.send(AddressPayload {
            payload,
            ..Default::default()
        }) {
            info!("failed to Hard Stop for {} error = {:?}", uuid.clone(), e);
        }
        info!("Successfully hard Stopped cast for {} ", uuid.clone());
    }
    Ok(warp::reply::json(&"ok"))
}


#[utoipa::path(get, path = "/ping")]
#[instrument(name = "ping")]
pub async fn ping_handler() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&"pong"))
}


struct Retry {
    retry_count: i8,
}

struct CastStreamWithRetry<T> {
    inner: ReusableBoxFuture<'static, (Result<T, RecvError>, Receiver<T>)>,
    channels: Arc<Mutex<UuidChannels>>,
    address_client: Arc<Mutex<AddressClients>>,
    event_sender: UnboundedSender<String>,
    db_client: Arc<DbClient>,
    uuid: String,
    address: String,
    codec: String,
    mode: String,
    metadata: String,
    retry: Arc<Mutex<Retry>>,
}

/// An error returned from the inner stream of a [`CastStreamWithRetry`].
#[derive(Debug, PartialEq, Eq, Clone)]
enum BroadcastStreamRecvError {
    /// The receiver lagged too far behind. Attempting to receive again will
    /// return the oldest message still retained by the channel.
    ///
    /// Includes the number of skipped messages.
    Lagged(u64),
}

async fn make_future<T: Clone>(mut rx: Receiver<T>) -> (Result<T, RecvError>, Receiver<T>) {
    let result = rx.recv().await;
    (result, rx)
}

impl<T: 'static + Clone + Send> CastStreamWithRetry<T> {
    /// Create a new `BroadcastStream`.
    pub fn new(rx: Receiver<T>, channels: Arc<Mutex<UuidChannels>>,
               address_client: Arc<Mutex<AddressClients>>,
               event_sender: UnboundedSender<String>,
               db_client: Arc<DbClient>,
               uuid: String,
               address: String,
               codec: String,
               mode: String,
               metadata: String,
               retry: Arc<Mutex<Retry>>) -> Self {
        Self {
            inner: ReusableBoxFuture::new(make_future(rx)),
            channels,
            address_client,
            event_sender,
            db_client,
            uuid,
            address,
            codec,
            mode,
            metadata,
            retry,
        }
    }
}

impl<T: 'static + Clone + Send> Stream for CastStreamWithRetry<T> {
    type Item = Result<T, BroadcastStreamRecvError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, rx) = ready!(self.inner.poll(cx));
        self.inner.set(make_future(rx));
        match result {
            Ok(item) => Poll::Ready(Some(Ok(item))),
            Err(RecvError::Closed) => Poll::Ready(None),
            Err(RecvError::Lagged(n)) => {
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(n))))
            }
        }
    }
}

impl<T> Drop for CastStreamWithRetry<T> {
    fn drop(&mut self) {
        let retry = self.retry.lock().unwrap();
        if retry.retry_count != -1 && retry.retry_count < 4 {
            let duration = u64::pow(2, retry.retry_count.clone() as u32) * 100;
            sleep(Duration::from_millis(duration));
            let db_client = self.db_client.clone();
            retry.retry_count = retry.retry_count + 1;
            if let Ok(_) = db_client.select_by_call_id_and_address(self.uuid.clone(), self.address.clone()) {
                start_cast(self.channels.clone(), self.address_client.clone(), self.event_sender.clone(), self.db_client.clone(),
                           self.uuid.clone(), self.address.clone(), self.codec.clone(),
                           self.mode.clone(), self.metadata.clone(), false, self.retry.clone());
            } else {
                start_cast(self.channels.clone(), self.address_client.clone(), self.event_sender.clone(), self.db_client.clone(),
                           self.uuid.clone(), self.address.clone(), self.codec.clone(),
                           self.mode.clone(), self.metadata.clone(), true, self.retry.clone());
            }
        } else {
            info!("Ignoring as retry exceeded");
        }
    }
}