// src/auth/username.rs

use crate::connection::SharedContacts;
use crate::utils::color::{ RED, RESET };
use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf };
use tokio::net::TcpStream;
use tokio::net::tcp::{ OwnedReadHalf, OwnedWriteHalf };

// 函数签名使用具体类型，使其更易于调用和理解
pub async fn validate_and_register_username(
    writer: &mut OwnedWriteHalf,
    reader: &mut BufReader<OwnedReadHalf>,
    contact: &SharedContacts
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut line = String::new();
    loop {
        line.clear();
        writer.write_all(b"Enter username: \n").await?;
        writer.flush().await?;

        if reader.read_line(&mut line).await? == 0 {
            // 客户端在登录时断开连接，这是一个明确的错误/退出条件
            return Err("Client disconnected during login".into());
        }

        let username = line.trim().to_string();
        if username.is_empty() {
            let err_msg = format!("{RED}Username cannot be empty.{RESET}\n");
            writer.write_all(err_msg.as_bytes()).await?;
            continue;
        }

        // 使用一个作用域来限制锁的持有时间
        let is_unique = {
            let guard = contact.lock().unwrap();
            !guard.contains_key(&username)
        };

        if is_unique {
            // 成功找到唯一用户名，返回
            return Ok(username);
        } else {
            let err_msg =
                format!("{RED}Username '{}' is taken, please try another.{RESET}\n", username);
            writer.write_all(err_msg.as_bytes()).await?;
        }
    }
}
