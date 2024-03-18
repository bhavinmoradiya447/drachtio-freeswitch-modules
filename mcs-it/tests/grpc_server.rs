use std::{collections::HashMap, fs::File, io::Write};
use std::string::ToString;
use log::info;
use serde_json::to_string;
use tokio;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use uuid::Uuid;
use mcs::{DialogRequestPayload, DialogRequestPayloadType, DialogResponsePayload, DialogResponsePayloadType};
use mcs::media_cast_service_server::{MediaCastService, MediaCastServiceServer};

pub mod mcs {
    tonic::include_proto!("mcs");
}

pub static UUID_FAILED_DIALOG: &str = "8b7afb6c-dbfc-478a-a773-ccad30b2b37e";
pub static UUID_SEND_EVENT: &str = "d8b005c8-082e-4f19-a2a1-423719037c1a";
pub static UUID_SEND_AUDIO: &str = "d7a61810-cc43-4617-a592-67892415b634";

pub struct MediaCastServiceImpl {}

#[tonic::async_trait]
impl MediaCastService for MediaCastServiceImpl {
    type DialogStream = ReceiverStream<Result<DialogResponsePayload, Status>>;

    async fn dialog(
        &self,
        request: Request<Streaming<DialogRequestPayload>>,
    ) -> Result<Response<Self::DialogStream>, Status> {
        info!("=========================== GOT CONNECTION =====================");
        let mut stream = request.into_inner();

        // create a receiver stream to return
        let (tx, rx) = tokio::sync::mpsc::channel(4);


        tokio::spawn(async move {
            while let Some(payload) = stream.message().await.unwrap() {
                if payload.payload_type == <DialogRequestPayloadType as Into<i32>>::into(
                    DialogRequestPayloadType::AudioStart,
                ) {
                    Self::send_response(tx.clone(), &payload);
                } else if payload.payload_type == <DialogRequestPayloadType as Into<i32>>::into(
                    DialogRequestPayloadType::EventData,
                ) {
                    if payload.event_data.len() > 0 {
                        info!("grpc_server -- got event data: {}", payload.event_data);
                        let response = DialogResponsePayload {
                            payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                                DialogResponsePayloadType::Event,
                            ),
                            audio: Vec::new(),
                            data: payload.event_data,
                        };
                        tx.send(Ok(response)).await.unwrap();
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

impl MediaCastServiceImpl {
    fn send_response(tx: Sender<Result<DialogResponsePayload, Status>>, payload: &DialogRequestPayload) {
        if let Ok(uuid) = Uuid::parse_str(payload.uuid.as_str()) {
            tokio::spawn(async move {
                if uuid.to_string().as_str().eq(UUID_FAILED_DIALOG) {
                    let response = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::DialogEnd,
                        ),
                        audio: Vec::new(),
                        data: String::from("Failed to connect upstream"),
                    };
                    tx.send(Ok(response)).await.unwrap();
                } else if uuid.to_string().as_str().eq(UUID_SEND_EVENT) {
                    let response = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::Event,
                        ),
                        audio: Vec::new(),
                        data: String::from("{\"Connection\":\"Success\"}"),
                    };
                    tx.send(Ok(response)).await.unwrap();
                } else if uuid.to_string().as_str().eq(UUID_SEND_EVENT) {
                    let response = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::AudioChunk,
                        ),
                        audio: vec![255; 320],
                        data: String::from("test_audio"),
                    };
                    tx.send(Ok(response.clone())).await.unwrap();
                    tx.send(Ok(response.clone())).await.unwrap();
                    tx.send(Ok(response.clone())).await.unwrap();
                    tx.send(Ok(response.clone())).await.unwrap();
                    tx.send(Ok(response.clone())).await.unwrap();
                    let response_end = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::ResponseEnd,
                        ),
                        audio: vec![255; 320],
                        data: String::from("test_audio"),
                    };
                    tx.send(Ok(response_end)).await.unwrap();
                } else {
                    let response = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::ResponseEnd,
                        ),
                        audio: Vec::new(),
                        data: String::from("Success"),
                    };
                    tx.send(Ok(response)).await.unwrap();
                }
            });
        }
    }
}


pub async fn start_grpc_server() -> Result<(), Box<dyn std::error::Error>> {
    info!("####################### Starting test grpc server ###################");
    Server::builder()
        .add_service(MediaCastServiceServer::new(MediaCastServiceImpl {}))
        .serve("0.0.0.0:50052".parse().unwrap())
        .await.expect("TODO: panic message");
    info!("####################### Started test grpc server ###################");

    Ok(())
}
