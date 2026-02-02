use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::error::{FlashFindError, Result};
use crate::persistence::get_app_data_dir;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Auto-save interval in seconds (0 = disabled)
    pub auto_save_interval: u64,
    
    /// Theme preference
    pub theme: Theme,
    
    /// Enabled drive letters (e.g., vec!['C', 'D'])
    pub enabled_drives: Vec<char>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Theme {
    Dark,
    Light,
    System,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_save_interval: 300, // 5 minutes
            theme: Theme::Dark,
            enabled_drives: vec!['C'], // Default: C drive only
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        
        if !path.exists() {
            info!("No config file found, using defaults");
            return Ok(Self::default());
        }
        
        let data = std::fs::read_to_string(&path).map_err(|e| {
            warn!("Failed to read config: {}", e);
            FlashFindError::FileReadError {
                path: path.display().to_string(),
                source: e,
            }
        })?;
        
        let config: Config = serde_json::from_str(&data).map_err(|e| {
            warn!("Failed to parse config: {}", e);
            FlashFindError::InvalidConfig(format!("Parse error: {}", e))
        })?;
        
        debug!("Loaded config from {}", path.display());
        Ok(config)
    }
    
    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        
        let data = serde_json::to_string_pretty(self).map_err(|e| {
            FlashFindError::InvalidConfig(format!("Serialization error: {}", e))
        })?;
        
        std::fs::write(&path, data).map_err(|e| FlashFindError::FileWriteError {
            path: path.display().to_string(),
            source: e,
        })?;
        
        info!("Saved config to {}", path.display());
        Ok(())
    }
    
    /// Get the configuration file path
    fn config_path() -> Result<PathBuf> {
        let app_dir = get_app_data_dir()?;
        Ok(app_dir.join("config.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.auto_save_interval, 300);
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.enabled_drives, vec!['C']);
    }
}
