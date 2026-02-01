use thiserror::Error;

/// Comprehensive error types for FlashFind operations
#[derive(Error, Debug)]
#[allow(dead_code)]
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

    #[error("Failed to insert path into index: {0}")]
    InsertionFailed(String),

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
    #[error("Operation timed out after {0} seconds")]
    Timeout(u64),

    #[error("Background thread panicked: {0}")]
    ThreadPanic(String),

    // Configuration Errors
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    // Serialization Errors
    #[error("Unsupported index version: {found}, expected: {expected}")]
    VersionMismatch { found: u32, expected: u32 },

    // Permission Errors
    #[error("Insufficient permissions to access: {0}")]
    PermissionDenied(String),

    // System Errors
    #[error("Failed to get system folder: {0}")]
    SystemFolderError(String),

    #[error("Out of memory while indexing")]
    OutOfMemory,

    // Operation Errors
    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}

/// Result type alias for FlashFind operations
pub type Result<T> = std::result::Result<T, FlashFindError>;

impl FlashFindError {
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            FlashFindError::Timeout(_)
                | FlashFindError::Cancelled
                | FlashFindError::InvalidQuery(_)
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
            FlashFindError::OutOfMemory => {
                "Out of memory. Try excluding large directories or reducing index size.".to_string()
            }
            FlashFindError::PermissionDenied(path) => {
                format!("Cannot access '{}'. Permission denied.", path)
            }
            FlashFindError::WatcherInitError(_) => {
                "Cannot monitor file changes. Real-time updates disabled.".to_string()
            }
            _ => self.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = FlashFindError::IndexFull(1000000);
        assert!(err.user_message().contains("1000000"));
        assert!(err.is_recoverable() == false);
    }

    #[test]
    fn test_recoverable_errors() {
        assert!(FlashFindError::Cancelled.is_recoverable());
        assert!(FlashFindError::Timeout(30).is_recoverable());
        assert!(!FlashFindError::OutOfMemory.is_recoverable());
    }
}
