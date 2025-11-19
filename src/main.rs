// src/main.rs

// 声明项目的顶层模块。Rust会据此查找对应的文件或目录。

mod auth;
mod connection;
mod message;
mod test;
mod utils;
// 引入需要的类型和函数
use crate::connection::SharedContacts;
use crate::utils::color::{ GREEN, RED, RESET };
use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use tokio::net::TcpListener;

use tracing::{ Level, error, info, warn };
use tracing_subscriber::FmtSubscriber;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing 日志系统
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO) // 设置默认日志级别
        .with_ansi(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // 初始化共享状态
    let contact: SharedContacts = Arc::new(Mutex::new(HashMap::new()));
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    //println!("{GREEN}Chat server started on 127.0.0.1:8080{RESET}");
    //info!("{GREEN}Chat server started on 127.0.0.1:8080{RESET}"); // tracing 宏在处理这个字符串时，会对其进行转义，以防止恶意的格式化字符串注入
    //info!
    info!("Chat server started on 127.0.0.1:8080");
    loop {
        // 等待新的客户端连接
        let (socket, addr) = match listener.accept().await {
            Ok(res) => res,
            Err(e) => {
                //eprintln!("{RED}Failed to accept connection: {}{RESET}", e);
                error!("{RED}Failed to accept connection: {}{RESET}", e);

                continue;
            }
        };

        //info!("{GREEN}>>> New client connected from: {}{RESET}", addr);
        info!(peer_addr = %addr, "New client connected"); // 使用结构化方式
        // 为新连接克隆共享状态的Arc指针
        let contact_clone = Arc::clone(&contact);

        // 为每个连接创建一个独立的异步任务
        tokio::spawn(async move {
            // 使用正确的、解耦后的函数路径
            if
                let Err(e) = connection::client::handle_connection(
                    socket,
                    addr,
                    contact_clone
                ).await
            {
                // 如果是 IO 错误（如 Broken pipe），则记录为警告
                // 其他错误记录为错误
                if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                    warn!(peer_addr = %addr, error = %io_err, "Connection handler finished with an I/O error");
                } else {
                    error!(peer_addr = %addr, error = ?e, "Connection handler failed with an unexpected error");
                }
                //eprintln!("{RED}Error handling connection from {}: {}{RESET}", addr, e);
            }
        });
    }
}
