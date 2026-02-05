use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::error::{FlashFindError, Result};
use crate::index::FileIndex;

/// Filesystem watcher that monitors directories for changes
pub struct Watcher {
    watcher: RecommendedWatcher,
    watched_dirs: Vec<PathBuf>,
}

impl Watcher {
    /// Create a new watcher with the given index
    pub fn new(index: Arc<RwLock<FileIndex>>) -> Result<Self> {
        info!("Initializing filesystem watcher");
        
        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => handle_fs_event(event, &index),
                Err(e) => error!("Watcher error: {}", e),
            }
        })
        .map_err(FlashFindError::WatcherInitError)?;
        
        Ok(Self {
            watcher,
            watched_dirs: Vec::new(),
        })
    }
    
    /// Watch a directory recursively
    pub fn watch_directory(&mut self, path: PathBuf) -> Result<()> {
        if !path.exists() {
            warn!("Cannot watch non-existent directory: {}", path.display());
            return Ok(()); // Don't fail, just skip
        }
        
        if !path.is_dir() {
            return Err(FlashFindError::InvalidPath(
                format!("{} is not a directory", path.display())
            ));
        }
        
        self.watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| FlashFindError::WatchError {
                path: path.display().to_string(),
                source: e,
            })?;
        
        info!("Watching directory: {}", path.display());
        self.watched_dirs.push(path);
        Ok(())
    }
    
    /// Clear all watched directories
    pub fn clear_watches(&mut self) {
        info!("Clearing {} watched directories", self.watched_dirs.len());
        self.watched_dirs.clear();
    }
    
    /// Watch multiple directories
    pub fn watch_directories(&mut self, paths: Vec<PathBuf>) -> Result<Vec<FlashFindError>> {
        // Clear existing watches to avoid duplicates
        self.clear_watches();
        
        let mut errors = Vec::new();
        
        for path in paths {
            if let Err(e) = self.watch_directory(path) {
                if !e.is_recoverable() {
                    return Err(e);
                }
                errors.push(e);
            }
        }
        
        Ok(errors)
    }
    
    /// Get list of currently watched directories (used in settings)
    pub fn watched_directories(&self) -> &[PathBuf] {
        &self.watched_dirs
    }
}

/// Handle filesystem events and update the index
fn handle_fs_event(event: Event, index: &Arc<RwLock<FileIndex>>) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            for path in event.paths {
                // Check permissions before processing
                if !has_read_permission(&path) {
                    debug!("Skipping file without read permission: {}", path.display());
                    continue;
                }
                
                if path.is_file() && !is_excluded(&path) && !is_temp_file(&path) {
                    debug!("File created/modified: {}", path.display());
                    
                    // Verify file is stable (not being written) before indexing
                    if !is_file_stable(&path) {
                        debug!("File not stable, skipping: {}", path.display());
                        continue;
                    }
                    
                    let mut lock = index.write();
                    match lock.insert(path.clone()) {
                        Ok(true) => debug!("Added to index: {}", path.display()),
                        Ok(false) => {}, // Duplicate, ignore
                        Err(e) => {
                            if !e.is_recoverable() {
                                error!("Failed to insert file: {}", e);
                            }
                        }
                    }
                }
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                debug!("File removed: {}", path.display());
                
                let mut lock = index.write();
                match lock.remove(&path) {
                    Ok(true) => debug!("Removed from index: {}", path.display()),
                    Ok(false) => {}, // Not in index
                    Err(e) => warn!("Failed to remove file: {}", e),
                }
            }
        }
        _ => {}
    }
}

/// Check if a file is stable (not currently being written)
fn is_file_stable(path: &Path) -> bool {
    use std::thread;
    use std::time::Duration;
    
    // Get initial metadata
    let size1 = match std::fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(_) => return false, // File doesn't exist or can't be read
    };
    
    // Wait briefly
    thread::sleep(Duration::from_millis(100));
    
    // Check again
    let size2 = match std::fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(_) => return false,
    };
    
    // If size is the same, file is likely stable
    size1 == size2
}

/// Check if a file is temporary or should be ignored
fn is_temp_file(path: &Path) -> bool {
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    // Common temporary file patterns
    filename.starts_with("~$")           // Office temp files
        || filename.starts_with(".~")    // Various temp files
        || filename.ends_with(".tmp")    // Generic temp
        || filename.ends_with(".temp")
        || filename.ends_with(".crdownload") // Chrome downloads
        || filename.ends_with(".part")   // Firefox downloads
        || filename.ends_with(".download") // Generic downloads
        || filename.contains(".tmp.")    // Embedded temp markers
}

/// Check if a path should be excluded from indexing
pub fn is_excluded(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // System directories to exclude
    let excluded = [
        "$recycle.bin",
        "appdata\\local",
        "appdata\\locallow", 
        "node_modules",
        ".git",
        ".svn",
        ".hg",
        "__pycache__",
        "target\\debug",    // Rust build artifacts
        "target\\release",
        ".vs",              // Visual Studio
        ".vscode",
        "bin\\debug",       // .NET build artifacts
        "bin\\release",
        "obj",
        "packages",         // NuGet packages
        "bower_components",
        ".cache",
        "temp",
        "tmp",
        "windows\\temp",
        "windows\\winsxs", // Windows component store (huge)
        "windows\\installer",
        "programdata\\microsoft", // System data
    ];
    
    for pattern in &excluded {
        if path_str.contains(pattern) {
            return true;
        }
    }
    
    // Exclude hidden files (starting with .)
    if let Some(filename) = path.file_name() {
        let filename_str = filename.to_string_lossy();
        if filename_str.starts_with('.') && filename_str != "." && filename_str != ".." {
            return true;
        }
    }
    
    // Exclude system files
    if path_str.ends_with(".sys") || 
       path_str.ends_with(".dll") ||
       path_str.ends_with(".tmp") {
        return true;
    }
    
    false
}

/// Get default directories to index based on Windows user folders
pub fn get_default_directories() -> Vec<PathBuf> {
    get_directories_for_drives(&['C'])
}

/// Get available Windows drive letters
pub fn get_available_drives() -> Vec<char> {
    let mut drives = Vec::new();
    
    #[cfg(target_os = "windows")]
    {
        // Check common drive letters A-Z
        for letter in 'A'..='Z' {
            let drive_path = format!("{}:\\", letter);
            if std::path::Path::new(&drive_path).exists() {
                drives.push(letter);
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Non-Windows: just return root
        drives.push('/');
    }
    
    drives
}

/// Get directories for specified drives
pub fn get_directories_for_drives(drive_letters: &[char]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    
    #[cfg(target_os = "windows")]
    {
        use known_folders::{get_known_folder_path, KnownFolder};
        
        // Only add user folders if C: drive is enabled
        if drive_letters.contains(&'C') {
            let folders = vec![
                (KnownFolder::Documents, "Documents"),
                (KnownFolder::Downloads, "Downloads"),
                (KnownFolder::Desktop, "Desktop"),
                (KnownFolder::Pictures, "Pictures"),
                (KnownFolder::Videos, "Videos"),
                (KnownFolder::Music, "Music"),
            ];
            
            for (folder, name) in folders {
                if let Some(path) = get_known_folder_path(folder) {
                    if path.exists() {
                        info!("Added default directory: {} ({})", name, path.display());
                        dirs.push(path);
                    } else {
                        warn!("Known folder {} does not exist: {}", name, path.display());
                    }
                } else {
                    warn!("Could not get path for known folder: {}", name);
                }
            }
        }
        
        // Add root of other enabled drives (excluding C:)
        for &drive in drive_letters {
            if drive != 'C' {
                let drive_root = PathBuf::from(format!("{}:\\", drive));
                if drive_root.exists() {
                    info!("Added drive root: {}", drive_root.display());
                    dirs.push(drive_root);
                }
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Fallback for non-Windows systems
        if let Ok(home) = std::env::var("HOME") {
            let home = PathBuf::from(home);
            for dir in &["Documents", "Downloads", "Desktop"] {
                let path = home.join(dir);
                if path.exists() {
                    dirs.push(path);
                }
            }
        }
    }
    
    if dirs.is_empty() {
        warn!("No default directories found!");
    }
    
    dirs
}

/// Check if we have read permission for a path
pub fn has_read_permission(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(_) => true,
        Err(e) => {
            use std::io::ErrorKind;
            match e.kind() {
                ErrorKind::PermissionDenied => {
                    debug!("Permission denied: {}", path.display());
                    false
                }
                ErrorKind::NotFound => {
                    debug!("Path not found: {}", path.display());
                    false
                }
                _ => {
                    warn!("Error accessing {}: {}", path.display(), e);
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclusion_patterns() {
        assert!(is_excluded(Path::new("C:\\$Recycle.Bin\\file.txt")));
        assert!(is_excluded(Path::new("C:\\Users\\Test\\AppData\\Local\\file.txt")));
        assert!(is_excluded(Path::new("C:\\project\\node_modules\\package.json")));
        assert!(is_excluded(Path::new("C:\\project\\.git\\config")));
        assert!(!is_excluded(Path::new("C:\\Users\\Test\\Documents\\file.txt")));
    }

    #[test]
    fn test_hidden_files() {
        assert!(is_excluded(Path::new("C:\\Users\\Test\\.hidden")));
        assert!(!is_excluded(Path::new("C:\\Users\\Test\\visible.txt")));
    }

    #[test]
    fn test_system_files() {
        assert!(is_excluded(Path::new("C:\\Windows\\System32\\driver.sys")));
        assert!(is_excluded(Path::new("C:\\Program Files\\app.dll")));
        assert!(!is_excluded(Path::new("C:\\Users\\Test\\document.pdf")));
    }
}
