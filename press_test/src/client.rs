// press_test/src/client.rs

use crate::config::TestStep;
use crate::metrics::{LocalMetrics, SharedMetrics};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::error;

pub async fn run_client(
    client_id: usize,
    host: String,
    port: u16,
    step: TestStep,
    metrics: SharedMetrics,
) {
    let mut local_metrics = LocalMetrics::default();

    let addr = format!("{}:{}", host, port);
    let stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            error!("Client {} failed to connect: {}", client_id, e);
            local_metrics.login_failures += 1;
            // 修复: 直接调用封装好的 merge 方法
            metrics.merge(local_metrics);
            return;
        }
    };
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // 1. 登录握手
    let username = format!("TestUser-{}", client_id);

    if reader.read_line(&mut line).await.is_err() {
        local_metrics.login_failures += 1;
        metrics.merge(local_metrics);
        return;
    }
    line.clear();

    if writer
        .write_all(format!("{}\n", username).as_bytes())
        .await
        .is_err()
    {
        local_metrics.login_failures += 1;
        metrics.merge(local_metrics);
        return;
    }

    if let Ok(bytes_read) = reader.read_line(&mut line).await {
        if bytes_read == 0 || line.contains("已被占用") {
            local_metrics.login_failures += 1;
            metrics.merge(local_metrics);
            return;
        }
    } else {
        local_metrics.login_failures += 1;
        metrics.merge(local_metrics);
        return;
    }
    line.clear();

    // 2. 消息收发循环
    let start_time = Instant::now();
    let duration = Duration::from_secs(step.duration_secs);
    let mut rng = SmallRng::from_entropy();
    let (min_think, max_think) = (step.think_time_ms[0], step.think_time_ms[1]);

    while start_time.elapsed() < duration {
        let think_time = rng.gen_range(min_think..=max_think);
        tokio::time::sleep(Duration::from_millis(think_time)).await;

        let msg = format!("hello from {}", client_id);
        let send_time = Instant::now();

        if writer
            .write_all(format!("{}\n", msg).as_bytes())
            .await
            .is_err()
        {
            local_metrics.send_errors += 1;
            break;
        }
        local_metrics.messages_sent += 1;

        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let latency = send_time.elapsed().as_micros() as u64;
                // 忽略记录失败的情况 (对于高精度直方图，失败概率极低)
                let _ = local_metrics.latencies.record(latency);
                local_metrics.messages_received += 1;
                line.clear();
            }
            Err(_) => break,
        }
    }

    // 3. 循环结束，将本地数据合并到全局
    metrics.merge(local_metrics);
}
