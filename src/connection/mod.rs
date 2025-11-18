// src/connection/mod.rs

// 声明 client.rs 是 connection 模块的一部分
// `pub` 关键字使其对外部模块（如 main.rs）可见
pub mod client;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// 将 ClientInfo 公开，以便 client.rs 和其他模块可以使用
pub struct ClientInfo {
    pub addr: SocketAddr,
    pub username: String,
    pub tx: mpsc::Sender<String>,
}

pub type SharedContacts = Arc<Mutex<HashMap<String, ClientInfo>>>;