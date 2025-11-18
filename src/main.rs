// src/main.rs

// 声明项目的顶层模块。Rust会据此查找对应的文件或目录。

mod auth;
mod connection;
mod message;
mod utils;
mod test;
// 引入需要的类型和函数
use crate::connection::SharedContacts;
use crate::utils::color::{ GREEN, RED, RESET };
use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化共享状态
    let contact: SharedContacts = Arc::new(Mutex::new(HashMap::new()));
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    println!("{GREEN}Chat server started on 127.0.0.1:8080{RESET}");

    loop {
        // 等待新的客户端连接
        let (socket, addr) = match listener.accept().await {
            Ok(res) => res,
            Err(e) => {
                eprintln!("{RED}Failed to accept connection: {}{RESET}", e);
                continue;
            }
        };

        println!("{GREEN}>>> New client connected from: {}{RESET}", addr);

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
                eprintln!("{RED}Error handling connection from {}: {}{RESET}", addr, e);
            }
        });
    }
}
