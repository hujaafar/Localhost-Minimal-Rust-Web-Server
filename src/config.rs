use serde::Deserialize;
use std::{collections::HashMap, fs};

#[derive(Debug, Deserialize, Clone)]
pub struct Redirect { pub status: u16, pub to: String }

#[derive(Debug, Deserialize, Clone)]
pub struct CgiCfg { pub ext: String, pub runner: String }

#[derive(Debug, Deserialize, Clone)]
pub struct RouteCfg {
    pub path: String,
    #[serde(default)] pub root: String,
    #[serde(default)] pub index: Vec<String>,
    #[serde(default)] pub methods: Vec<String>,
    #[serde(default)] pub dir_listing: bool,
    #[serde(default)] pub upload_enabled: bool,
    #[serde(default)] pub redirect: Option<Redirect>,
    #[serde(default)] pub cgi: Option<CgiCfg>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerCfg {
    pub server_address: String,
    pub ports: Vec<u16>,
    #[serde(default)] pub server_name: Vec<String>,
    #[serde(default)] pub client_max_body_size: usize,
    #[serde(default)] pub error_pages: HashMap<String, String>,
    pub routes: Vec<RouteCfg>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub servers: Vec<ServerCfg>,
    #[serde(default = "default_timeout")] pub request_timeout_ms: u64,
    #[serde(default = "default_max_events")] pub epoll_max_events: i32,
}

fn default_timeout() -> u64 { 5000 }
fn default_max_events() -> i32 { 1024 }

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let s = fs::read_to_string(path)?;
        let cfg: Config = serde_json::from_str(&s)?;
        Ok(cfg)
    }
}
