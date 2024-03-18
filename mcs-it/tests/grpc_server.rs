use std::{collections::HashMap, fs::File, io::Write};
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

static UUID_FAILED_DIALOG: Uuid = Uuid::new_v4();
static UUID_FAILED_SEND_EVENT: Uuid = Uuid::new_v4();
static UUID_FAILED_SEND_AUDIO: Uuid = Uuid::new_v4();

pub struct MediaCastServiceImpl {}

#[tonic::async_trait]
impl MediaCastService for MediaCastServiceImpl {
    type DialogStream = ReceiverStream<Result<DialogResponsePayload, Status>>;

    async fn dialog(
        &self,
        request: Request<Streaming<DialogRequestPayload>>,
    ) -> Result<Response<Self::DialogStream>, Status> {
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
                        println!("[info] got event data: {}", payload.event_data);
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
                if uuid.eq(&UUID_FAILED_DIALOG) {
                    let response = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::DialogEnd,
                        ),
                        audio: Vec::new(),
                        data: String::from("Failed to connect upstream"),
                    };
                    tx.send(Ok(response)).await.unwrap();
                } else if uuid.eq(&UUID_FAILED_SEND_EVENT) {
                    let response = DialogResponsePayload {
                        payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                            DialogResponsePayloadType::Event,
                        ),
                        audio: Vec::new(),
                        data: String::from("{\"Connection\":\"Success\"}"),
                    };
                    tx.send(Ok(response)).await.unwrap();
                } else if uuid.eq(&UUID_FAILED_SEND_EVENT) {
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
        .await?;
    Ok(())
}
