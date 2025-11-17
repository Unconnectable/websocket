// press_test/src/metrics.rs

use chrono::Local;
use hdrhistogram::Histogram;
use serde::Serialize;
use std::fs;
use std::sync::{ Arc, Mutex };
use std::time::Duration;

// 报告中用于展示延迟分布的结构体
#[derive(Debug, Serialize, Default, Clone)]
pub struct LatencyReport {
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

// 最终生成JSON报告的核心结构体
#[derive(Debug, Serialize, Default, Clone)]
pub struct FinalReport {
    pub step_name: String,
    pub test_duration_secs: f64,
    pub concurrency: usize,
    pub total_sent: u64,
    pub total_received: u64,
    pub total_login_failures: u64,
    pub total_send_errors: u64,
    pub send_tps: f64,
    pub receive_tps: f64,
    pub latency: LatencyReport,
}

// --- 内部使用的指标收集器 ---

// 单个客户端线程本地的指标，以减少全局锁争用
#[derive(Debug)]
pub struct LocalMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub login_failures: u64,
    pub send_errors: u64,
    // 使用HDR直方图高效记录延迟，单位为微秒 (u64)
    pub latencies: Histogram<u64>,
}

impl Default for LocalMetrics {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            login_failures: 0,
            send_errors: 0,
            // 初始化直方图，精度为3位有效数字
            latencies: Histogram::new(3).unwrap(),
        }
    }
}

// 全局线程安全的指标聚合器
#[derive(Debug)]
pub struct GlobalMetrics {
    // 使用Mutex来允许多线程安全访问
    inner: Mutex<GlobalMetricsInner>,
}

#[derive(Debug)]
struct GlobalMetricsInner {
    pub total_sent: u64,
    pub total_received: u64,
    pub total_login_failures: u64,
    pub total_send_errors: u64,
    // 所有客户端的延迟数据都会合并到这个全局直方图中
    pub combined_latencies: Histogram<u64>,
}

impl GlobalMetrics {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(GlobalMetricsInner {
                total_sent: 0,
                total_received: 0,
                total_login_failures: 0,
                total_send_errors: 0,
                // 配置直方图记录范围：1微秒 到 30秒，精度3位
                combined_latencies: Histogram::new_with_bounds(1, 30_000_000, 3).unwrap(),
            }),
        }
    }

    /// 将单个客户端的本地指标合并到全局聚合器中
    pub fn merge(&self, local: LocalMetrics) {
        let mut guard = self.inner.lock().unwrap();
        guard.total_sent += local.messages_sent;
        guard.total_received += local.messages_received;
        guard.total_login_failures += local.login_failures;
        guard.total_send_errors += local.send_errors;
        // 使用 `add` 方法安全地合并两个直方图
        guard.combined_latencies.add(local.latencies).unwrap();
    }

    /// 从聚合的指标生成最终的、可序列化的报告
    pub fn generate_final_report(
        &self,
        step_name: &str,
        concurrency: usize,
        duration: Duration
    ) -> FinalReport {
        let guard = self.inner.lock().unwrap();
        let total_seconds = duration.as_secs_f64();

        let mut latency_report = LatencyReport::default();
        if guard.combined_latencies.len() > 0 {
            latency_report = LatencyReport {
                mean_ms: guard.combined_latencies.mean() / 1000.0,
                p50_ms: (guard.combined_latencies.value_at_quantile(0.5) as f64) / 1000.0,
                p95_ms: (guard.combined_latencies.value_at_quantile(0.95) as f64) / 1000.0,
                p99_ms: (guard.combined_latencies.value_at_quantile(0.99) as f64) / 1000.0,
                max_ms: (guard.combined_latencies.max() as f64) / 1000.0,
            };
        }

        FinalReport {
            step_name: step_name.to_string(),
            test_duration_secs: total_seconds,
            concurrency,
            total_sent: guard.total_sent,
            total_received: guard.total_received,
            total_login_failures: guard.total_login_failures,
            total_send_errors: guard.total_send_errors,
            send_tps: if total_seconds > 0.0 {
                (guard.total_sent as f64) / total_seconds
            } else {
                0.0
            },
            receive_tps: if total_seconds > 0.0 {
                (guard.total_received as f64) / total_seconds
            } else {
                0.0
            },
            latency: latency_report,
        }
    }
}

pub type SharedMetrics = Arc<GlobalMetrics>;

/// 将最终报告保存为带时间戳的JSON文件
pub fn save_report_to_json(report: &FinalReport) -> Result<(), Box<dyn std::error::Error>> {
    let dir = "reports";
    fs::create_dir_all(dir)?;

    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    // 清理步骤名，使其适合做文件名
    let safe_step_name = report.step_name.replace(|c: char| !c.is_alphanumeric(), "_");
    let filename = format!(
        "{}/{}_{}_{}c_{}s.json",
        dir,
        timestamp,
        safe_step_name,
        report.concurrency,
        report.test_duration_secs.round()
    );

    let report_json = serde_json::to_string_pretty(report)?;
    fs::write(&filename, report_json)?;

    println!("\n✅ Report saved to {}", filename);
    Ok(())
}
