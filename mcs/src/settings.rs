use std::fmt;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Log {
    pub level: String,
    pub log_dir: String,
    pub log_prefix: String,
    pub appender: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EslClient {
    pub host: String,
    pub auth: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FsHttpClient {
    pub user_name: String,
    pub password: String
}


#[derive(Debug, Deserialize, Clone)]
pub struct HttpServer {
    pub port: u16,
    pub url: String,
    pub token_file: String,
    pub grpc_connection_pool: usize,
    pub tls_cert_file: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UdpServer {
    pub socket_file: String,
}

#[derive(Clone, Debug, Deserialize)]
pub enum ENV {
    Development,
    Testing,
    Production,
}

impl fmt::Display for ENV {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ENV::Development => write!(f, "Development"),
            ENV::Testing => write!(f, "Testing"),
            ENV::Production => write!(f, "Production"),
        }
    }
}

impl From<&str> for ENV {
    fn from(env: &str) -> Self {
        match env {
            "Testing" => ENV::Testing,
            "Production" => ENV::Production,
            _ => ENV::Development,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub log: Log,
    pub http_server: HttpServer,
    pub udp_server: UdpServer,
    pub env: ENV,
    pub fs_esl_client: EslClient,
    pub fs_http_client: FsHttpClient
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let env = std::env::var("RUN_ENV").unwrap_or_else(|_| "Development".into());
        let mut setting = Config::new();
        setting.set("env", env.clone())?;

        setting.merge(File::with_name(CONFIG_FILE_PATH))?;
        setting.merge(File::with_name(&format!("{}{}.toml", CONFIG_FILE_PREFIX, env.to_lowercase())))?;

        // This makes it so "EA_SERVER__PORT overrides server.port
        setting.merge(Environment::with_prefix("ea").separator("__"))?;

        setting.try_into()
    }
}

const CONFIG_FILE_PATH: &str = "./config/default.toml";
const CONFIG_FILE_PREFIX: &str = "./config/";
