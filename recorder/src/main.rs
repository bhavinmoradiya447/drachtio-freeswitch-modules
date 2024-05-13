use std::{collections::HashMap, fs::File, io::Write};
use tokio;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::media_cast_service_server::MediaCastService;
use crate::mcs::media_cast_service_server::MediaCastServiceServer;
use crate::mcs::DialogRequestPayload;
use crate::mcs::DialogRequestPayloadType;
use crate::mcs::DialogResponsePayload;
use crate::mcs::DialogResponsePayloadType;

pub struct MediaCastServiceImpl {}

#[tonic::async_trait]
impl MediaCastService for MediaCastServiceImpl {
    type DialogStream = ReceiverStream<Result<DialogResponsePayload, Status>>;

    async fn dialog(
        &self,
        request: Request<Streaming<DialogRequestPayload>>,
    ) -> Result<Response<Self::DialogStream>, Status> {
        let mut stream = request.into_inner();

        // create a map of uuid to file
        let mut files = HashMap::new();
        let mut left_files = HashMap::new();
        let mut right_files = HashMap::new();

        tokio::spawn(async move {
            while let Some(payload) = stream.message().await.unwrap() {
                // println!("[debug] seq: {}, size: {}", payload.seq, payload.size);
                // if uuid does not exist in map create file
                if payload.event_data.len() > 0 {
                    println!("[info] got event data: {}", payload.event_data);
                }
                if !files.contains_key(&payload.uuid) {
                    // create file
                    println!("[info] opening file: /tmp/rec-{}.raw", payload.uuid);
                    let mut file = File::create(format!("/tmp/rec-{}.raw", payload.uuid)).unwrap();
                    let mut left_file =
                        File::create(format!("/tmp/rec-{}-left.raw", payload.uuid)).unwrap();
                    let mut right_file =
                        File::create(format!("/tmp/rec-{}-right.raw", payload.uuid)).unwrap();
                    file.write_all(payload.audio.as_slice()).unwrap();
                    left_file.write_all(&payload.audio_left).unwrap();
                    right_file.write_all(&payload.audio_right).unwrap();
                    files.insert(payload.uuid.clone(), file);
                    left_files.insert(payload.uuid.clone(), left_file);
                    right_files.insert(payload.uuid, right_file);
                } else {
                    if payload.payload_type
                        == <DialogRequestPayloadType as Into<i32>>::into(
                            DialogRequestPayloadType::AudioStop,
                        )
                    {
                        // close file
                        println!("[info] closing file: /tmp/rec-{}.raw", payload.uuid);
                        let mut file = files.remove(&payload.uuid).unwrap();
                        file.flush().unwrap();
                        let mut left_file = left_files.remove(&payload.uuid).unwrap();
                        left_file.flush().unwrap();
                        let mut right_file = right_files.remove(&payload.uuid).unwrap();
                        right_file.flush().unwrap();
                    } else {
                        // append audio to file
                        // println!("[trace] writing seq: {}, size: {} to file: /tmp/rec-{}.raw", payload.seq, payload.audio.len(), payload.uuid);
                        let mut file = files.get(&payload.uuid).unwrap();
                        file.write_all(payload.audio.as_slice()).unwrap();
                        let mut left_file = left_files.get(&payload.uuid).unwrap();
                        left_file.write_all(&payload.audio_left).unwrap();
                        let mut right_file = right_files.get(&payload.uuid).unwrap();
                        right_file.write_all(&payload.audio_right).unwrap();
                    }
                }
            }
        });

        // create a receiver stream to return
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let response = DialogResponsePayload {
                payload_type: <DialogResponsePayloadType as Into<i32>>::into(
                    DialogResponsePayloadType::ResponseEnd,
                ),
                audio: Vec::new(),
                data: String::from("recording started"),
            };
            tx.send(Ok(response)).await.unwrap();
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting recorder service...");
    Server::builder()
        .add_service(MediaCastServiceServer::new(MediaCastServiceImpl {}))
        .serve("0.0.0.0:50051".parse().unwrap())
        .await?;
    Ok(())
}
