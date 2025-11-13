use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, BufReader, AsyncBufReadExt};
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;

// 引入可发送的随机数生成器
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::Rng; // 仍需要 Rng trait

// 结构体用于收集单个客户端的性能数据 (保持不变)
#[derive(Debug, Default)]
pub struct ClientMetrics {
    pub messages_sent: usize,
    pub messages_received: usize,
    pub total_latency_ms: u128,
}

// 共享的全局统计数据 (保持不变)
pub type SharedStats = Arc<Mutex<ClientMetrics>>;

// 修复 2 & 3 的类型错误: 将返回的 Error 类型修改为 Box<dyn std::error::Error + Send + 'static>
// 这样就能满足 tokio::spawn 的 Send 约束。
pub async fn run_client(
    host: String,
    port: u16,
    client_id: usize,
    duration: Duration,
    send_interval_ms: (u64, u64),
    stats: SharedStats,
) -> Result<(), Box<dyn std::error::Error + Send + 'static>> { // <-- 修复 2: 显式添加 Send 约束

    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(&addr).await?;
    let (reader_stream, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader_stream);
    
    let username = format!("TestUser{}", client_id);
    let mut line = String::new();
    
    // 修复 3: 使用 SmallRng 并通过随机种子创建，使其可跨线程发送 (Send)
    let mut rng = SmallRng::from_entropy(); 

    // 1. 握手和认证 (保持不变)
    reader.read_line(&mut line).await?; 
    writer.write_all(format!("{}\n", username).as_bytes()).await?;
    reader.read_line(&mut line).await?; 
    line.clear();
    
    // 2. 消息发送与接收循环 (保持不变)
    let start_time = Instant::now();
    let mut metrics = ClientMetrics::default();
    
    let (min_send_ms, max_send_ms) = send_interval_ms;
    
    while start_time.elapsed() < duration {
        let interval_ms = rng.gen_range(min_send_ms..=max_send_ms);
        let next_send_wait = Duration::from_millis(interval_ms);
        
        tokio::select! {
            _ = tokio::time::sleep(next_send_wait) => { 
                let msg = format!("Hello from client {}", client_id);
                
                if let Err(_) = writer.write_all(format!("{}\n", msg).as_bytes()).await {
                    break; 
                }
                metrics.messages_sent += 1;
            }
            
            read_result = reader.read_line(&mut line) => {
                match read_result {
                    Ok(0) => break, 
                    Ok(_) => {
                        metrics.messages_received += 1;
                        line.clear();
                    }
                    Err(_) => break,
                }
            }
        }
    }

    // 3. 统计数据合并 (保持不变)
    let mut shared_stats = stats.lock().await;
    shared_stats.messages_sent += metrics.messages_sent;
    shared_stats.messages_received += metrics.messages_received;
    
    let _ = writer.write_all(b"/quit\n").await;

    Ok(())
}