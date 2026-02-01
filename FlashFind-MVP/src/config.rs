use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::error::{FlashFindError, Result};
use crate::persistence::get_app_data_dir;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directories to index
    pub watched_directories: Vec<PathBuf>,
    
    /// Auto-save interval in seconds (0 = disabled)
    pub auto_save_interval: u64,
    
    /// Maximum index size
    pub max_index_size: usize,
    
    /// Theme preference
    pub theme: Theme,
    
    /// Show hidden files
    pub show_hidden_files: bool,
    
    /// Custom exclusion patterns
    pub custom_exclusions: Vec<String>,
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
            watched_directories: Vec::new(), // Will be populated with defaults
            auto_save_interval: 300, // 5 minutes
            max_index_size: 10_000_000,
            theme: Theme::Dark,
            show_hidden_files: false,
            custom_exclusions: Vec::new(),
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
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.auto_save_interval, 300);
        assert_eq!(config.max_index_size, 10_000_000);
        assert_eq!(config.theme, Theme::Dark);
        assert!(!config.show_hidden_files);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.auto_save_interval, deserialized.auto_save_interval);
    }
}
