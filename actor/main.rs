// actor/main.rs

// 声明模块，文件名必须匹配
mod client;
mod hub;
mod models;

use crate::hub::Hub;
use crate::models::HubCommand;
use anyhow::Result;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{ Level, error, info, warn };
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).with_ansi(true).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // 1. 创建 Hub 的主通信通道
    // 这个通道容量可以设置大一点，作为整个服务器的“写入缓冲”
    let (hub_tx, hub_rx) = mpsc::channel::<HubCommand>(1000);

    // 2. 启动 Hub 任务 (Actor)
    let mut hub = Hub::new(hub_rx);
    tokio::spawn(async move {
        hub.run().await;
    });

    // 3. 启动 TCP 监听
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("🚀 Actor-based Chat Server started on 127.0.0.1:8080");

    // 4. 接收连接循环
    loop {
        let (socket, addr) = match listener.accept().await {
            Ok(res) => res,
            Err(e) => {
                error!(error = %e, "Failed to accept connection");
                continue;
            }
        };

        info!(peer_addr = %addr, "New connection established.");

        let hub_tx_clone = hub_tx.clone();

        // 5. 为每个连接启动一个 Client 任务
        tokio::spawn(async move {
            if let Err(e) = client::handle_connection(socket, addr, hub_tx_clone).await {
                // 区分 IO 错误和其他错误
                if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                    warn!(peer_addr = %addr, error = %io_err, "Client disconnected (IO error)");
                } else {
                    error!(peer_addr = %addr, error = ?e, "Client handler failed");
                }
            }
        });
    }
}
