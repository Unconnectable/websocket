// actor/hub.rs

use crate::models::{ Client, HubCommand, RegisterResult };
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::{ info, warn, error };

/// Hub 结构体，作为应用的状态和业务逻辑核心
pub struct Hub {
    /// Hub 从所有客户端任务接收命令的入口
    receiver: mpsc::Receiver<HubCommand>,
    /// 存储所有已连接的客户端信息，键为用户名
    clients: HashMap<String, Client>,
}

impl Hub {
    pub fn new(receiver: mpsc::Receiver<HubCommand>) -> Self {
        Hub {
            receiver,
            clients: HashMap::new(),
        }
    }

    /// 运行 Hub 的主事件循环。
    pub async fn run(&mut self) {
        info!("[Hub] Started processing commands.");
        while let Some(command) = self.receiver.recv().await {
            match command {
                HubCommand::Register { username, addr, sender, responder } =>
                    self.register(username, addr, sender, responder),

                HubCommand::Deregister { username } => self.deregister(&username),

                // 注意：这里不再需要 await，因为 broadcast 变成了同步非阻塞
                HubCommand::Broadcast { from, message } => self.broadcast(&from, &message),
            }
        }
        info!("[Hub] Channel closed, shutting down.");
    }

    fn register(
        &mut self,
        username: String,
        addr: SocketAddr,
        sender: mpsc::Sender<String>,
        responder: tokio::sync::oneshot::Sender<RegisterResult>
    ) {
        if self.clients.contains_key(&username) {
            // 忽略发送错误，因为客户端可能已经断开
            let _ = responder.send(RegisterResult::UsernameTaken);
        } else {
            let client = Client {
                username: username.clone(),
                addr,
                sender,
            };
            self.clients.insert(username.clone(), client);
            info!(
                username = %username,
                addr = %addr,
                total_clients = self.clients.len(),
                "[Hub] Client registered."
            );
            let _ = responder.send(RegisterResult::Success);
        }
    }

    fn deregister(&mut self, username: &str) {
        if self.clients.remove(username).is_some() {
            info!(
                username = %username,
                total_clients = self.clients.len(),
                "[Hub] Client deregistered."
            );
        }
    }

    // 🔥 修复：移除 async，使用 try_send 防止阻塞
    fn broadcast(&self, from: &str, message: &str) {
        let broadcast_msg = format!("[{}]: {}", from, message);
        // info!(from = %from, "[Hub] Broadcasting message."); // 可以根据需要开启 debug 日志

        for (username, client) in &self.clients {
            if username != from {
                // 使用 try_send，如果某个客户端队列满了，直接丢弃消息或报错，
                // 绝不让 Hub 等待（await）。
                match client.sender.try_send(broadcast_msg.clone()) {
                    Ok(_) => {}
                    Err(TrySendError::Full(_)) => {
                        warn!(to = %username, "[Hub] Client queue is full! Dropping message.");
                    }
                    Err(TrySendError::Closed(_)) => {
                        // 客户端已断开，通常会在 Deregister 中清理，这里可以忽略
                        // 或者在这里记录一个 debug 日志
                    }
                }
            }
        }
    }
}
