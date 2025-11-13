use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TestStep {
    pub concurrency: usize,
    pub duration_secs: u64,
    pub send_interval_ms: (u64, u64),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TestConfig {
    pub host: String,
    pub port: u16,
    pub steps: Vec<TestStep>,
}

pub fn load_config(path: &str) -> Result<TestConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: TestConfig = toml::from_str(&content)?;
    Ok(config)
}