// actor/client.rs

use crate::models::{ HubCommand, RegisterResult };
use anyhow::{ Result, bail };
use std::net::SocketAddr;
use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, BufReader };
use tokio::net::TcpStream;
use tokio::sync::{ mpsc, oneshot };
use tracing::{ info, warn };

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

pub async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    hub_tx: mpsc::Sender<HubCommand>
) -> Result<()> {
    let (reader_stream, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader_stream);

    // 🔥 修复步骤1：提前创建好属于这个客户端的接收通道
    // 这样我们才能把 tx 交给 Hub，把 rx 留给自己用
    let (client_tx, mut client_rx) = mpsc::channel::<String>(100);

    // --- 异步用户名验证循环 ---
    let username = loop {
        writer.write_all(b"Enter username: \n").await?;
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            bail!("Client disconnected during login");
        }
        let name_attempt = line.trim().to_string();

        if name_attempt.is_empty() {
            writer.write_all(format!("{RED}Username cannot be empty.{RESET}\n").as_bytes()).await?;
            continue;
        }

        // 准备一次性的回复通道
        let (resp_tx, resp_rx) = oneshot::channel();

        // 🔥 修复步骤2：将真正的 client_tx 发送给 Hub
        let cmd = HubCommand::Register {
            username: name_attempt.clone(),
            addr,
            sender: client_tx.clone(), // <--- 这里传的是真货！
            responder: resp_tx,
        };

        if hub_tx.send(cmd).await.is_err() {
            bail!("Hub has been shutdown.");
        }

        match resp_rx.await {
            Ok(RegisterResult::Success) => {
                break name_attempt;
            }
            Ok(RegisterResult::UsernameTaken) => {
                writer.write_all(
                    format!("{RED}Username '{}' is taken.{RESET}\n", name_attempt).as_bytes()
                ).await?;
                continue;
            }
            Err(_) => bail!("Hub dropped the request (shutdown?)."),
        }
    };

    // --- 注册成功 ---
    info!(username = %username, peer_addr = %addr, "User session started.");
    writer.write_all(format!("{GREEN}Welcome, {}!{RESET}\n", username).as_bytes()).await?;

    let mut line = String::new();

    // --- 主事件循环 ---
    loop {
        tokio::select! {
            // 1. 处理来自网络的消息（读取客户端输入 -> 发给 Hub）
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let message = line.trim().to_string();
                        if !message.is_empty() {
                            let cmd = HubCommand::Broadcast { 
                                from: username.clone(), 
                                message 
                            };
                            if hub_tx.send(cmd).await.is_err() {
                                break; // Hub 挂了
                            }
                        }
                        line.clear();
                    }
                    Err(e) => {
                        warn!(username = %username, error = %e, "Error reading from socket");
                        break;
                    }
                }
            }
            
            // 2. 处理来自 Hub 的消息（读取 Channel -> 写入网络）
            // 这里使用的是上面创建的 client_rx
            Some(msg) = client_rx.recv() => {
                if writer.write_all(msg.as_bytes()).await.is_err() {
                    break; 
                }
                if writer.write_all(b"\n").await.is_err() {
                    break;
                }
            }
        }
    }

    // --- 清理工作 ---
    // 尝试通知 Hub 注销。如果 Hub 已经关闭或发送失败，我们也不在乎了。
    let _ = hub_tx.send(HubCommand::Deregister {
        username: username.clone(),
    }).await;

    info!(username = %username, "User session finished.");

    Ok(())
}
