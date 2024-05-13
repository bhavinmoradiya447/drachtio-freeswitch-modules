use std::time::Duration;
use httpclient::{Client, ResponseExt};
use tracing::error;
use crate::CONFIG;

#[derive(Debug)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            client: httpclient::Client::new()
        }
    }

    pub async fn is_call_leg_exist(&self, uuid: String) -> bool {
        if CONFIG.env.to_string().eq_ignore_ascii_case("development") {
            true
        } else {
            let request_url = &format!("http://127.0.0.1:7080/xmlapi/uuid_exists?{}", uuid);
            match self.client.get(request_url).basic_auth("ZnJlZXN3aXRjaDpRckZkVEtEM20kYjc5OVBx")
                .send().await {
                Ok(res) => {
                    match res.status().is_success() {
                        true => {
                            match res.text().await {
                                Ok(body) => {
                                    match body.as_str() {
                                        "true" => { true }
                                        _ => false
                                    }
                                }
                                Err(e) => {
                                    error!("Error reading response body {:?}", e);
                                    false
                                }
                            }
                        }
                        false => { false }
                    }
                }
                Err(e) => {
                    error!("Error sending request to fs {:?}", e);
                    false
                }
            }
        }
    }
}