use std::time::Duration;
use reqwest::Client;
use tracing::{error, info};
use crate::CONFIG;

#[derive(Debug)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_millis(5000))
                .http2_keep_alive_while_idle(true)
                .http2_prior_knowledge()
                .http2_keep_alive_timeout(Duration::from_millis(5000))
                .http2_keep_alive_interval(Some(Duration::from_millis(10000)))
                .build().unwrap()
        }
    }

    pub async fn is_call_leg_exist(&self, uuid: String) -> bool {
        info!("UserName {}, password {}", CONFIG.fs_http_client.user_name.clone(), Some(CONFIG.fs_http_client.password.clone());
        let request_url = format!("http://127.0.0.1:7080/xmlapi/uuid_exists?{}", uuid);
        match self.client.get(request_url)
            .timeout(Duration::from_secs(3))
            .basic_auth(CONFIG.fs_http_client.user_name.clone(), Some(CONFIG.fs_http_client.password.clone())).send().await {
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