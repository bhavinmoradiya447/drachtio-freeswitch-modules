use std::time::Duration;
use tracing::{error, info};
use crate::CONFIG;
use base64::prelude::BASE64_STANDARD;
use base64::write::EncoderWriter;
use std::io::Write;
use tracing::log::trace;

#[derive(Debug)]
pub struct HttpClient;

impl HttpClient {
    pub fn is_call_leg_exist(&self, uuid: String) -> bool {
        info!("Checking if call leg id {} present", uuid.clone());
        let result = if CONFIG.env.to_string().eq_ignore_ascii_case("development") {
            true
        } else {
            let request_url = format!("http://127.0.0.1:7080/xmlapi/uuid_exists?{}", uuid);

            let res = ureq::get(request_url.as_str())
                .set("Authorization", format!("Basic {}",
                                              basic_auth(CONFIG.fs_http_client.user_name.clone(),
                                                         Some(CONFIG.fs_http_client.password.clone()))
                                                  .as_str()).as_str()).call();

            match res {
                Ok(res) => {
                    match res.status() >= 200 && res.status() < 300 {
                        true => {
                            match res.into_string() {
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
        };
        info!("uuid {} present {}", uuid.clone(), result );
        result
    }
}

fn basic_auth<U, P>(username: U, password: Option<P>) -> String
    where
        U: std::fmt::Display,
        P: std::fmt::Display,
{
    let mut buf = b"".to_vec();
    {
        let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
        let _ = write!(encoder, "{username}:");
        if let Some(password) = password {
            let _ = write!(encoder, "{password}");
        }

        let res = encoder.finish().unwrap();
        String::from_utf8(res.to_vec()).unwrap()
    }
}