// press_test/src/main.rs

mod client;
mod config;
mod metrics;

use crate::config::load_config;
use crate::metrics::{
    save_report_to_json,
    GlobalMetrics,
    SharedMetrics,
    StepReport,
    TestRunReport,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use tracing_subscriber::{ fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter };

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 初始化日志系统 (新设计) ---
    let file_appender = tracing_appender::rolling::daily("logs", "press_test.log");
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);

    // 创建一个写入文件的日志层，并强制禁用ANSI颜色代码
    let file_layer = fmt::layer().with_writer(non_blocking_writer).with_ansi(false);

    // 创建一个写入控制台的日志层，保持默认的ANSI颜色行为
    let console_layer = fmt::layer();

    tracing_subscriber
        ::registry()
        .with(EnvFilter::from_default_env().add_directive("press_test=info".parse()?))
        .with(file_layer) // 添加文件层
        .with(console_layer) // 添加控制台层
        .init();

    let config = load_config("config.toml")?;
    info!("--- 🚀 Starting Chat Server Performance Test ---");
    let target_server = format!("{}:{}", config.host, config.port);
    info!("Target: {}\n", target_server);

    // 创建一个向量来收集所有步骤的报告
    let mut step_reports: Vec<StepReport> = Vec::new();

    // --- 按顺序执行所有测试步骤 ---
    for step in config.steps {
        info!("--- ▶️ Running Step: '{}' ---", step.name);
        info!("Concurrency: {}, Duration: {}s", step.concurrency, step.duration_secs);

        let global_metrics: SharedMetrics = Arc::new(GlobalMetrics::new());
        let mut handles = Vec::new();
        let step_start_time = Instant::now();

        for i in 0..step.concurrency {
            let host = config.host.clone();
            let port = config.port;
            let step_clone = step.clone();
            let metrics_clone = global_metrics.clone();

            let handle = tokio::spawn(async move {
                client::run_client(i, host, port, step_clone, metrics_clone).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await?;
        }

        let elapsed_duration = step_start_time.elapsed();

        // 生成单个步骤的报告
        let step_report = global_metrics.generate_step_report(
            &step.name,
            step.concurrency,
            elapsed_duration
        );

        // 打印简短总结到控制台
        println!("\n--- Summary for Step: '{}' ---", step_report.step_name);
        println!("Test Duration: {:.2}s", step_report.test_duration_secs);
        println!("Receive TPS: {:.2}", step_report.receive_tps);
        println!("P95 Latency: {:.3}ms", step_report.latency.p95_ms);
        info!("--- ✅ Step '{}' Finished ---\n", step.name);

        // 将该步骤的报告存入向量
        step_reports.push(step_report);
    }

    // --- 所有步骤完成后，创建并保存最终的聚合报告 ---
    let final_run_report = TestRunReport {
        target_server,
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        steps: step_reports,
    };

    if let Err(e) = save_report_to_json(&final_run_report) {
        tracing::error!("Failed to save final report: {}", e);
    }

    info!("--- 🎉 All test steps completed! ---");

    Ok(())
}
