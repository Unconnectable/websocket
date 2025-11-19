// press_test/src/metrics.rs

use chrono::{DateTime, Utc};
use hdrhistogram::Histogram;
use serde::Serialize;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// 最终的顶层报告结构，对应一个完整的测试运行
#[derive(Debug, Serialize)]
pub struct TestRunReport {
    pub target_server: String,
    // 使用 RFC 3339 格式的时间戳，这是标准格式
    pub timestamp_utc: String,
    // 一个数组，包含本次运行中所有步骤的结果
    pub steps: Vec<StepReport>,
}

// 单个测试步骤的报告结构 (之前叫 FinalReport)
#[derive(Debug, Serialize, Default, Clone)]
pub struct StepReport {
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

#[derive(Debug, Serialize, Default, Clone)]
pub struct LatencyReport {
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

// --- 内部使用的指标收集器 (这部分 API 不变) ---

#[derive(Debug)]
pub struct LocalMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub login_failures: u64,
    pub send_errors: u64,
    pub latencies: Histogram<u64>,
}

impl Default for LocalMetrics {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            login_failures: 0,
            send_errors: 0,
            latencies: Histogram::new(3).unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct GlobalMetrics {
    inner: Mutex<GlobalMetricsInner>,
}

#[derive(Debug)]
struct GlobalMetricsInner {
    pub total_sent: u64,
    pub total_received: u64,
    pub total_login_failures: u64,
    pub total_send_errors: u64,
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
                combined_latencies: Histogram::new_with_bounds(1, 30_000_000, 3).unwrap(),
            }),
        }
    }

    pub fn merge(&self, local: LocalMetrics) {
        let mut guard = self.inner.lock().unwrap();
        guard.total_sent += local.messages_sent;
        guard.total_received += local.messages_received;
        guard.total_login_failures += local.login_failures;
        guard.total_send_errors += local.send_errors;
        guard.combined_latencies.add(local.latencies).unwrap();
    }

    // 这个函数现在只生成单个步骤的报告
    pub fn generate_step_report(
        &self,
        step_name: &str,
        concurrency: usize,
        duration: Duration,
    ) -> StepReport {
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

        StepReport {
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

// 这个函数现在接收顶层报告并保存
pub fn save_report_to_json(report: &TestRunReport) -> Result<(), Box<dyn std::error::Error>> {
    let dir = "reports";
    fs::create_dir_all(dir)?;

    // 使用UTC时间的RFC3339格式作为文件名的一部分，保证唯一且可排序
    let timestamp =
        DateTime::parse_from_rfc3339(&report.timestamp_utc)?.format("%Y-%m-%d_%H-%M-%S");

    let filename = format!("{}/run_{}.json", dir, timestamp);
    let report_json = serde_json::to_string_pretty(report)?;
    fs::write(&filename, report_json)?;

    println!("\n✅ Complete test report saved to {}", filename);
    Ok(())
}
