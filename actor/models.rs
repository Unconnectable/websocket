// actor/models.rs

use std::net::SocketAddr;
use tokio::sync::{ mpsc, oneshot };

/// 代表一个已连接的客户端的所有信息，由 Hub 持有
#[derive(Debug)]
pub struct Client {
    pub username: String,
    pub addr: SocketAddr,
    /// 这个 Sender 用于将消息（如广播）发回给该客户端的写入任务
    pub sender: mpsc::Sender<String>,
}

/// 定义客户端任务可以发送给 Hub 的所有命令
#[derive(Debug)]
pub enum HubCommand {
    /// 新客户端注册，包含一个用于回复注册结果的 oneshot channel
    Register {
        username: String,
        addr: SocketAddr,
        sender: mpsc::Sender<String>, // 这里必须传真的 Sender
        responder: oneshot::Sender<RegisterResult>,
    },
    /// 客户端断开连接
    Deregister {
        username: String,
    },
    /// 广播消息
    Broadcast {
        from: String,
        message: String,
    },
}

/// 注册操作的结果，通过 oneshot channel 返回
#[derive(Debug)]
pub enum RegisterResult {
    Success,
    UsernameTaken,
}
