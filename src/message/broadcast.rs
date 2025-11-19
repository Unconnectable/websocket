// src/message/brodcast.rs

use crate::connection::{ ClientInfo, SharedContacts };
use tokio::sync::mpsc::Sender;
pub async fn broadcast_to_others(contact: &SharedContacts, sender_username: &str, msg: String) {
    // 收集所有需要接收消息的客户端的 Sender
    // 使用一个独立的作用域来确保锁尽快被释放
    let format_msg = format!("[{sender_username}]: {msg}");
    let receivers: Vec<Sender<String>> = {
        let guard = contact.lock().unwrap();
        guard
            .iter()
            .filter(|(name, _)| *name != sender_username)
            .map(|(_, info)| info.tx.clone())
            .collect()
    };

    // 异步地将消息发送给所有接收者
    for tx in receivers {
        // 忽略发送错误，因为接收方可能已经下线
        let _ = tx.send(format_msg.clone()).await;
    }
}
