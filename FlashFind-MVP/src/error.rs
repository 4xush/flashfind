use thiserror::Error;

/// Error types for FlashFind operations
#[derive(Error, Debug)]
pub enum FlashFindError {
    // Filesystem & I/O Errors
    #[error("Failed to read file: {path}")]
    FileReadError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file: {path}")]
    FileWriteError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to create directory: {path}")]
    DirectoryCreationError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    // Index Errors
    #[error("Index is corrupted or in invalid format")]
    CorruptedIndex(#[from] bincode::Error),

    #[error("Index has reached maximum capacity of {0} files")]
    IndexFull(usize),

    // Watcher Errors
    #[error("Failed to initialize filesystem watcher")]
    WatcherInitError(#[from] notify::Error),

    #[error("Failed to watch directory: {path}")]
    WatchError {
        path: String,
        #[source]
        source: notify::Error,
    },

    // Concurrency Errors
    #[error("Background thread panicked: {0}")]
    ThreadPanic(String),

    // Configuration Errors
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    // Serialization Errors
    #[error("Unsupported index version: {found}, expected: {expected}")]
    VersionMismatch { found: u32, expected: u32 },

    // System Errors
    #[error("Failed to get system folder: {0}")]
    SystemFolderError(String),

    // Operation Errors
    #[error("Operation cancelled by user")]
    Cancelled,
}

/// Result type alias for FlashFind operations
pub type Result<T> = std::result::Result<T, FlashFindError>;

impl FlashFindError {
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            FlashFindError::Cancelled
                | FlashFindError::WatchError { .. }
        )
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            FlashFindError::IndexFull(max) => {
                format!("Index is full. Maximum {} files supported. Consider excluding more directories.", max)
            }
            FlashFindError::CorruptedIndex(_) => {
                "Index file is corrupted. It will be rebuilt.".to_string()
            }
            FlashFindError::WatcherInitError(_) => {
                "Cannot monitor file changes. Real-time updates disabled.".to_string()
            }
            _ => self.to_string(),
        }
    }
}


