use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use crate::error::{FlashFindError, Result};
use crate::index::{FileIndex, INDEX_VERSION};

/// Get the application data directory
pub fn get_app_data_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use known_folders::{get_known_folder_path, KnownFolder};
        
        let roaming_appdata = get_known_folder_path(KnownFolder::RoamingAppData)
            .ok_or_else(|| FlashFindError::SystemFolderError("APPDATA".to_string()))?;
        
        let app_dir = roaming_appdata.join("FlashFind");
        Ok(app_dir)
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Fallback for non-Windows (though this is a Windows-focused app)
        let home = std::env::var("HOME")
            .map_err(|_| FlashFindError::SystemFolderError("HOME".to_string()))?;
        Ok(PathBuf::from(home).join(".flashfind"))
    }
}

/// Get the path to the index file
pub fn get_index_path() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    
    // Ensure directory exists
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).map_err(|e| FlashFindError::DirectoryCreationError {
            path: app_dir.display().to_string(),
            source: e,
        })?;
        info!("Created application data directory: {}", app_dir.display());
    }
    
    Ok(app_dir.join("index.bin"))
}

/// Get the path to the log file
pub fn get_log_path() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).map_err(|e| FlashFindError::DirectoryCreationError {
            path: app_dir.display().to_string(),
            source: e,
        })?;
    }
    
    Ok(app_dir.join("flashfind.log"))
}

/// Load the index from disk with version checking
pub fn load_index() -> Result<FileIndex> {
    let path = get_index_path()?;
    
    if !path.exists() {
        info!("No existing index found at {}", path.display());
        return Ok(FileIndex::new());
    }
    
    debug!("Loading index from {}", path.display());
    
    let data = fs::read(&path).map_err(|e| FlashFindError::FileReadError {
        path: path.display().to_string(),
        source: e,
    })?;
    
    let mut index: FileIndex = bincode::deserialize(&data)
        .map_err(|e| {
            error!("Failed to deserialize index: {}", e);
            FlashFindError::CorruptedIndex(e)
        })?;
    
    // Version compatibility check
    if index.version() != INDEX_VERSION {
        warn!(
            "Index version mismatch: found {}, expected {}",
            index.version(),
            INDEX_VERSION
        );
        return Err(FlashFindError::VersionMismatch {
            found: index.version(),
            expected: INDEX_VERSION,
        });
    }
    
    // Rebuild runtime cache
    index.rebuild_cache();
    
    info!("Loaded index with {} files", index.len());
    Ok(index)
}

/// Save the index to disk atomically
/// 
/// This performs an atomic write by:
/// 1. Writing to a temporary file
/// 2. Renaming the temp file to the target (atomic operation on same filesystem)
pub fn save_index(index: &FileIndex) -> Result<()> {
    let path = get_index_path()?;
    let temp_path = path.with_extension("tmp");
    
    debug!("Saving index with {} files", index.len());
    
    // Serialize to bytes
    let data = bincode::serialize(index).map_err(|e| {
        error!("Failed to serialize index: {}", e);
        FlashFindError::CorruptedIndex(e)
    })?;
    
    // Write to temporary file
    fs::write(&temp_path, &data).map_err(|e| FlashFindError::FileWriteError {
        path: temp_path.display().to_string(),
        source: e,
    })?;
    
    // Atomic rename (overwrites existing file)
    fs::rename(&temp_path, &path).map_err(|e| FlashFindError::FileWriteError {
        path: path.display().to_string(),
        source: e,
    })?;
    
    info!("Index saved successfully to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_app_data_dir() {
        let result = get_app_data_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("FlashFind"));
    }

    #[test]
    fn test_get_index_path() {
        let result = get_index_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().ends_with("index.bin"));
    }
}
