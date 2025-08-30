use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfraredConfig {
    pub infrared_output_value_minimum: f32,
    pub infrared_output_value_maximum: f32,
    pub infrared_source_scale: f32,
}

impl Default for InfraredConfig {
    fn default() -> Self {
        Self {
            infrared_output_value_minimum: 0.25,
            infrared_output_value_maximum: 1.0,
            infrared_source_scale: 3.0,
        }
    }
}

pub struct InfraredConfigManager {
    config: Arc<RwLock<InfraredConfig>>,
    config_path: String,
    last_modified: Arc<RwLock<Option<SystemTime>>>,
}

impl InfraredConfigManager {
    pub fn new(config_path: String) -> Result<Self> {
        let config = Self::load_config(&config_path)?;
        let last_modified = Self::get_file_modified_time(&config_path)?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            last_modified: Arc::new(RwLock::new(Some(last_modified))),
        })
    }

    pub fn get_config(&self) -> InfraredConfig {
        self.config.read().clone()
    }

    pub fn check_and_reload(&self) -> Result<bool> {
        let current_modified = Self::get_file_modified_time(&self.config_path)?;
        let last_modified = *self.last_modified.read();

        if let Some(last) = last_modified {
            if current_modified > last {
                log::info!("📄 Infrared config file changed, reloading...");
                let new_config = Self::load_config(&self.config_path)?;
                *self.config.write() = new_config.clone();
                *self.last_modified.write() = Some(current_modified);

                log::info!(
                    "✅ Infrared config reloaded: min={:.2}, max={:.2}, scale={:.1}",
                    new_config.infrared_output_value_minimum,
                    new_config.infrared_output_value_maximum,
                    new_config.infrared_source_scale
                );
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn load_config(config_path: &str) -> Result<InfraredConfig> {
        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path))?;

        let config: InfraredConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path))?;

        // Validate config values
        if config.infrared_output_value_minimum < 0.0 || config.infrared_output_value_minimum > 1.0
        {
            anyhow::bail!("infrared_output_value_minimum must be between 0.0 and 1.0");
        }
        if config.infrared_output_value_maximum < 0.0 || config.infrared_output_value_maximum > 1.0
        {
            anyhow::bail!("infrared_output_value_maximum must be between 0.0 and 1.0");
        }
        if config.infrared_output_value_minimum >= config.infrared_output_value_maximum {
            anyhow::bail!(
                "infrared_output_value_minimum must be less than infrared_output_value_maximum"
            );
        }
        if config.infrared_source_scale <= 0.0 {
            anyhow::bail!("infrared_source_scale must be greater than 0.0");
        }

        Ok(config)
    }

    fn get_file_modified_time(path: &str) -> Result<SystemTime> {
        let metadata = fs::metadata(path)
            .with_context(|| format!("Failed to get metadata for file: {}", path))?;

        metadata
            .modified()
            .with_context(|| format!("Failed to get modified time for file: {}", path))
    }

    pub fn spawn_config_monitor(self: Arc<Self>) {
        std::thread::spawn(move || {
            log::info!("🔄 Starting infrared config monitor (checking every 3 seconds)...");

            loop {
                std::thread::sleep(Duration::from_secs(1));

                if let Err(e) = self.check_and_reload() {
                    log::warn!("⚠️ Error checking config file: {}", e);
                }
            }
        });
    }
}
