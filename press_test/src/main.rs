use clap::Parser;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::process;
use tracing_subscriber::{EnvFilter, fmt};

mod client;
mod config;
use client::{run_client, ClientMetrics};
use config::{load_config, TestConfig};

const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

// 使用 clap 宏定义命令行参数，只指定配置文件路径
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
//async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
    
    let args = Args::parse();
    
    let config: TestConfig = match load_config(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("读取配置失败: {}. 请确保 '{}' 文件存在。", e, args.config);
            process::exit(1);
        }
    };

    println!("--- 🚀 性能测试启动 ---");
    println!("目标: {}:{}", config.host, config.port);
    
    let total_start_time = Instant::now();
    let mut step_count = 1;

    // 核心逻辑：顺序执行配置文件中的所有测试步骤
    for step in config.steps {
        println!("\n--- 🏁 开始测试步骤 {} ---", step_count);
        println!("并发数: {}", step.concurrency);
        println!("时长: {}s", step.duration_secs);
        
        let test_duration = Duration::from_secs(step.duration_secs);
        let global_stats = Arc::new(Mutex::new(ClientMetrics::default()));
        let mut handles = Vec::new();
        let step_start_time = Instant::now();

        // 启动所有客户端任务
        for i in 0..step.concurrency {
            let host = config.host.clone();
            let stats_clone = global_stats.clone();
            let step_clone = step.clone();
            
            let handle = tokio::spawn(run_client(
                host,
                config.port,
                i,
                test_duration,
                step_clone.send_interval_ms,
                stats_clone,
            ));
            handles.push(handle);
        }

        println!("所有 {} 个客户端已启动...", step.concurrency);
        
        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await; 
        }

        let elapsed_time = step_start_time.elapsed();
        let total_seconds = elapsed_time.as_secs_f64();

        // 汇总结果并计算指标
        let final_stats = global_stats.lock().await;
        let total_sent = final_stats.messages_sent;
        let total_received = final_stats.messages_received;
        
        let sent_tps = (total_sent as f64) / total_seconds;
        let received_tps = (total_received as f64) / total_seconds;

        println!("--- ✅ 步骤 {} 结果 ---", step_count);
        println!("测试总时长: {:.2} 秒", total_seconds);
        println!("并发连接数: {}", step.concurrency);
        println!("---------------------");
        println!("总发送消息数: {}", total_sent);
        println!("总接收消息数: {}", total_received);
        println!("吞吐量 (发送): {:.2} TPS", sent_tps);
        println!("吞吐量 (接收/广播): {:.2} TPS", received_tps);
        // 总转发量 = 总发送量 * (并发数 - 1)
        println!("总转发消息数 (估计): {} (发送量 * 并发数 - 1)", total_sent.saturating_mul(step.concurrency.saturating_sub(1)));
        
        if total_sent == 0 {
            eprintln!("\n{RED}警告：没有消息被发送，请检查服务器是否正在运行。{RESET}");
        }
        
        step_count += 1;
    }

    println!("\n--- 🎉 所有测试步骤完成。总耗时: {:.2} 秒 ---", total_start_time.elapsed().as_secs_f64());
    Ok(())
}