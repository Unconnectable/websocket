// src/connection/client.rs

use super::{ ClientInfo, SharedContacts }; // `super` 指向父模块 connection
use crate::auth::username::validate_and_register_username;
use crate::message::broadcast::broadcast_to_others;
use crate::utils::color::{ GREEN, RESET };
use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, BufReader };
use tokio::net::TcpStream;
use tokio::sync::mpsc; // <--- 添加 mpsc 的 use 语句

use tracing::info;
use anyhow::Result;

pub async fn handle_connection(
    socket: TcpStream,
    addr: std::net::SocketAddr,
    contact: SharedContacts
) -> anyhow::Result<()> {
    let (reader_stream, mut writer) = socket.into_split();
    let mut reader: BufReader<tokio::net::tcp::OwnedReadHalf> = BufReader::new(reader_stream);

    // 1. 进行用户名验证和获取
    let username = validate_and_register_username(&mut writer, &mut reader, &contact).await?;

    // 2. 为此客户端创建消息通道
    let (tx, mut rx) = mpsc::channel(100);

    // 3. 验证成功后，将客户端信息（包括其消息通道）注册到全局状态中
    {
        let mut guard = contact.lock().unwrap();
        let client_info = ClientInfo {
            addr,
            username: username.clone(),
            tx, // 这个 tx 是上面新创建的
        };
        guard.insert(username.clone(), client_info);
    }
    info!(username = %username, peer_addr = %addr, "User registered successfully.");
    //info!("{GREEN}User '{}' (from {}) registered successfully.{RESET}", username, addr);
    writer.write_all(format!("{GREEN}Welcome, {}!{RESET}\n", username).as_bytes()).await?;

    // 4. 进入主事件循环
    let mut line = String::new();
    loop {
        tokio::select! {
            // 从客户端读取一行输入
            result = reader.read_line(&mut line) => {
                let bytes_read = result?;
                if bytes_read == 0 {
                    // 客户端主动断开连接 (EOF)
                    break;
                }

                // 将消息广播给其他人
                broadcast_to_others(&contact, &username, line.trim().to_string()).await;
                line.clear();
            }
            // 从其他人的广播中接收消息
            Some(msg_to_receive) = rx.recv() => {
                writer.write_all(msg_to_receive.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
    }

    // 5. 客户端断开连接后的清理工作
    contact.lock().unwrap().remove(&username);
    info!(
        "User '{}' disconnected. Active connections: {}",
        username,
        contact.lock().unwrap().len()
    );

    Ok(())
}
