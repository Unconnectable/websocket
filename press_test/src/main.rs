// press_test/src/main.rs

mod client;
mod config;
mod metrics;

use crate::config::load_config;
use crate::metrics::{save_report_to_json, GlobalMetrics, SharedMetrics};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 初始化日志系统 ---
    let file_appender = tracing_appender::rolling::daily("logs", "press_test.log");
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("press_test=info".parse()?))
        .with(fmt::layer().with_writer(non_blocking_writer))
        .with(fmt::layer())
        .init();

    // --- 加载配置 ---
    let config = load_config("config.toml")?;
    info!("--- 🚀 Starting Chat Server Performance Test ---");
    info!("Target: {}:{}\n", config.host, config.port);

    // --- 按顺序执行所有测试步骤 ---
    for step in config.steps {
        info!("--- ▶️ Running Step: '{}' ---", step.name);
        info!(
            "Concurrency: {}, Duration: {}s",
            step.concurrency, step.duration_secs
        );

        let global_metrics: SharedMetrics = Arc::new(GlobalMetrics::new());
        let mut handles = Vec::new();
        let step_start_time = Instant::now();

        // --- 并发启动所有虚拟客户端 ---
        for i in 0..step.concurrency {
            let host = config.host.clone();
            let port = config.port;
            let step_clone = step.clone();
            // 修复: 修正拼写错误
            let metrics_clone = global_metrics.clone();

            let handle = tokio::spawn(async move {
                client::run_client(i, host, port, step_clone, metrics_clone).await;
            });
            handles.push(handle);
        }

        // --- 等待所有客户端任务执行完毕 ---
        for handle in handles {
            handle.await?;
        }

        let elapsed_duration = step_start_time.elapsed();

        // --- 生成并保存报告 ---
        let final_report =
            global_metrics.generate_final_report(&step.name, step.concurrency, elapsed_duration);

        // 打印一个简短的总结到控制台
        println!("\n--- Summary for Step: '{}' ---", final_report.step_name);
        println!("Test Duration: {:.2}s", final_report.test_duration_secs);
        println!("Receive TPS: {:.2}", final_report.receive_tps);
        println!("P95 Latency: {:.3}ms", final_report.latency.p95_ms);

        if let Err(e) = save_report_to_json(&final_report) {
            tracing::error!("Failed to save report: {}", e);
        }

        info!("--- ✅ Step '{}' Finished ---\n", step.name);
    }

    Ok(())
}