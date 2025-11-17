// press_test/src/config.rs

use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct TestStep {
    pub name: String,
    pub concurrency: usize,
    pub duration_secs: u64,
    pub think_time_ms: [u64; 2],
}

#[derive(Debug, Deserialize)]
pub struct TestConfig {
    pub host: String,
    pub port: u16,
    pub steps: Vec<TestStep>,
}

pub fn load_config(path: &str) -> Result<TestConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: TestConfig = toml::from_str(&content)?;
    Ok(config)
}
