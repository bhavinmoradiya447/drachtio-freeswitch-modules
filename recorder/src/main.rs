
// use tokio::sync::mpsc;
use std::{io::Write, fs::File, collections::HashMap};
use tonic::{Request, Response, Status, transport::Server, Streaming};
use tokio;
// use uuid::Uuid;

pub mod mcs {
    tonic::include_proto!("mcs");
}

use crate::mcs::multi_cast_service_server::MultiCastServiceServer;
use crate::mcs::multi_cast_service_server::MultiCastService;
use crate::mcs::Payload;
use crate::mcs::ListenerResponse;

pub struct MultiCastServiceImpl {}

#[tonic::async_trait]
impl MultiCastService for MultiCastServiceImpl {
    async fn listen(
        &self,
        request: Request<Streaming<Payload>>,
    ) -> Result<Response<ListenerResponse>, Status> {
        let mut stream = request.into_inner();

        // create a map of uuid to file
        let mut files = HashMap::new();

        tokio::spawn(async move {
            while let Some(payload) = stream.message().await.unwrap() {
                // if uuid does not exist in map create file
                if !files.contains_key(&payload.uuid) {
                    // create file
                    println!("[info] opening file: /tmp/rec-{}.raw", payload.uuid);
                    let mut file = File::create(format!("/tmp/rec-{}.raw", payload.uuid)).unwrap();
                    file.write_all(payload.audio.as_slice()).unwrap();
                    files.insert(payload.uuid, file);
                } else {
                    if payload.size==0 {
                        // close file
                        println!("[info] empty seq: {}, closing file: /tmp/rec-{}.raw",payload.seq, payload.uuid);
                        let mut file = files.remove(&payload.uuid).unwrap();
                        file.flush().unwrap();
                    } else {
                        // append audio to file
                        println!("[trace] writing seq: {} to file: /tmp/rec-{}.raw", payload.seq, payload.uuid);
                        let mut file = files.get(&payload.uuid).unwrap();
                        file.write_all(payload.audio.as_slice()).unwrap();
                    }
                }

            }
        });

        let response = ListenerResponse {
            ok: true,
        };
        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting recorder service...");
    Server::builder()
        .add_service(MultiCastServiceServer::new(MultiCastServiceImpl {}))
        .serve("0.0.0.0:50051".parse().unwrap())
        .await?;
    Ok(())
}