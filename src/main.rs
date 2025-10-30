use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

// --- 核心类型定义 ---
// 值（Value）现在是 mpsc::Sender 的克隆体，它实现了 Clone
type SharedState = Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Chat server V0.1 listening on 127.0.0.1:8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!(">>> New client connected: {}", addr);

        let state_clone = state.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, addr, state_clone).await {
                // 通常只会在客户端意外断开或代码逻辑错误时发生
                eprintln!("Error handling connection from {}: {}", addr, e);
            }
        });
    }
}

// 客户端连接处理函数
async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 为这个新客户端创建一个 mpsc 通道（它的专属“收件箱”）
    let (tx, mut rx) = mpsc::channel(32);

    // 2. 注册到共享状态（通讯录）
    {
        // MutexGuard 在块结束时自动解锁
        let mut map = state.lock().unwrap();
        map.insert(addr, tx);
    }

    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        tokio::select! {
            // --- 分支 A: 从客户端读取输入 (等待 read_line) ---
            result = reader.read_line(&mut line) => {
                let bytes_read = match result {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("Error reading from {}: {}", addr, e);
                        break;
                    }
                };

                // 检查连接是否断开 (EOF)
                if bytes_read == 0 {
                    println!("<<< Client {} disconnected.", addr);
                    break;
                }

                // 拿到消息，去除首尾空白和换行
                let msg = line.trim().to_string();
                println!("[IN] {}: {}", addr, msg);

                // --- 广播逻辑（修正后的）---
                // 关键修正：在执行 .await 之前，释放 Mutex
                let senders: Vec<mpsc::Sender<String>> = {
                    let map = state.lock().unwrap(); // 🔒 Mutex 被锁定
                    // 收集所有 Sender 的克隆体
                    map.values().cloned().collect()
                }; // 🔒 Mutex 在这里（map 离开作用域时）被自动解锁！

                // 在 Mutex 解锁的情况下，执行 send().await
                for peer_tx in senders.into_iter() {
                    if let Err(e) = peer_tx.send(msg.clone()).await {
                        // 对方的 Receiver 已经被 drop 了，说明对方刚断开，忽略此错误
                        // 在 v0.2 中，我们可以根据此错误来清理死连接
                        eprintln!("Failed to send to a peer: {}", e);
                    }
                }

                line.clear();
            }

            // --- 分支 B: 从自己的收件箱接收广播消息 (等待 rx.recv) ---
            Some(msg) = rx.recv() => {
                writer.write_all(msg.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
    }

    // --- 5. 清理阶段：任务退出前执行 ---
    {
        let mut map = state.lock().unwrap();
        map.remove(&addr);
    }
    println!(
        "--- Client {} handler finished. Active connections: {}",
        addr,
        state.lock().unwrap().len()
    );

    Ok(())
}
