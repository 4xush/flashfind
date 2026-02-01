use crossbeam_channel::{bounded, Sender};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

use crate::error::{FlashFindError, Result};
use crate::index::FileIndex;
use crate::persistence::save_index;
use crate::watcher::is_excluded;

/// Indexing state and progress information
#[derive(Clone, Debug)]
pub enum IndexState {
    Idle,
    Scanning { progress: usize },
    Saving,
    Error { message: String },
}

/// Commands that can be sent to the indexer thread
pub enum IndexCommand {
    StartScan(Vec<PathBuf>),
}

/// Result of indexing operation
pub struct IndexResult {
    pub files_added: usize,
    pub duration_ms: u64,
}

/// Background indexer that scans directories without blocking the UI
pub struct Indexer {
    #[allow(dead_code)]
    index: Arc<RwLock<FileIndex>>,
    state: Arc<RwLock<IndexState>>,
    is_running: Arc<AtomicBool>,
    #[allow(dead_code)]
    cancel_flag: Arc<AtomicBool>,
    command_tx: Sender<IndexCommand>,
    #[allow(dead_code)]
    thread_handle: Option<JoinHandle<()>>,
}

impl Indexer {
    /// Create a new background indexer
    pub fn new(index: Arc<RwLock<FileIndex>>) -> Result<Self> {
        let (command_tx, command_rx) = bounded::<IndexCommand>(10);
        
        let state = Arc::new(RwLock::new(IndexState::Idle));
        let is_running = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        
        // Clone Arc references for the thread
        let thread_index = index.clone();
        let thread_state = state.clone();
        let thread_running = is_running.clone();
        let thread_cancel = cancel_flag.clone();
        
        // Spawn background thread
        let thread_handle = thread::spawn(move || {
            indexer_thread(
                thread_index,
                thread_state,
                thread_running,
                thread_cancel,
                command_rx,
            );
        });
        
        Ok(Self {
            index,
            state,
            is_running,
            cancel_flag,
            command_tx,
            thread_handle: Some(thread_handle),
        })
    }
    
    /// Start scanning directories
    pub fn start_scan(&self, directories: Vec<PathBuf>) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Indexing already in progress");
            return Ok(());
        }
        
        info!("Starting scan of {} directories", directories.len());
        self.command_tx
            .send(IndexCommand::StartScan(directories))
            .map_err(|_| FlashFindError::ThreadPanic("Indexer thread not responding".to_string()))?;
        
        Ok(())
    }
    
    /// Get current indexing state
    pub fn state(&self) -> IndexState {
        self.state.read().clone()
    }
    
    /// Check if indexing is currently running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
}

/// Background thread that handles indexing operations
fn indexer_thread(
    index: Arc<RwLock<FileIndex>>,
    state: Arc<RwLock<IndexState>>,
    is_running: Arc<AtomicBool>,
    cancel_flag: Arc<AtomicBool>,
    command_rx: crossbeam_channel::Receiver<IndexCommand>,
) {
    info!("Indexer thread started");
    
    loop {
        match command_rx.recv() {
            Ok(IndexCommand::StartScan(directories)) => {
                is_running.store(true, Ordering::Relaxed);
                cancel_flag.store(false, Ordering::Relaxed);
                *state.write() = IndexState::Scanning { progress: 0 };
                
                let result = scan_directories(
                    directories,
                    &index,
                    &state,
                    &cancel_flag,
                );
                
                match result {
                    Ok(stats) => {
                        info!(
                            "Scan completed: {} files added in {}ms",
                            stats.files_added, stats.duration_ms
                        );
                        
                        // Auto-save after successful scan
                        *state.write() = IndexState::Saving;
                        if let Err(e) = save_index(&*index.read()) {
                            error!("Failed to auto-save index: {}", e);
                            *state.write() = IndexState::Error {
                                message: e.user_message(),
                            };
                        } else {
                            *state.write() = IndexState::Idle;
                        }
                    }
                    Err(e) => {
                        error!("Scan failed: {}", e);
                        *state.write() = IndexState::Error {
                            message: e.user_message(),
                        };
                    }
                }
                
                is_running.store(false, Ordering::Relaxed);
            }
            
            Err(_) => {
                warn!("Command channel closed, shutting down");
                break;
            }
        }
    }
    
    info!("Indexer thread stopped");
}

/// Scan directories and add files to index
fn scan_directories(
    directories: Vec<PathBuf>,
    index: &Arc<RwLock<FileIndex>>,
    state: &Arc<RwLock<IndexState>>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<IndexResult> {
    let start_time = Instant::now();
    let mut total_added = 0;
    
    for dir in directories {
        if cancel_flag.load(Ordering::Relaxed) {
            info!("Scan cancelled");
            return Err(FlashFindError::Cancelled);
        }
        
        debug!("Scanning directory: {}", dir.display());
        
        // Collect all file paths without holding lock
        let entries: Vec<PathBuf> = WalkDir::new(&dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| !is_excluded(e.path()))
            .map(|e| e.into_path())
            .collect();
        
        debug!("Found {} files in {}", entries.len(), dir.display());
        
        // Batch insert with periodic lock releases
        const BATCH_SIZE: usize = 1000;
        for chunk in entries.chunks(BATCH_SIZE) {
            if cancel_flag.load(Ordering::Relaxed) {
                info!("Scan cancelled during batch insert");
                return Err(FlashFindError::Cancelled);
            }
            
            let mut lock = index.write();
            
            for path in chunk {
                match lock.insert(path.clone()) {
                    Ok(true) => total_added += 1,
                    Ok(false) => {}, // Duplicate
                    Err(e) => {
                        if !e.is_recoverable() {
                            return Err(e);
                        }
                        // Log but continue on recoverable errors
                        warn!("Failed to insert {}: {}", path.display(), e);
                    }
                }
            }
            
            // Update progress
            *state.write() = IndexState::Scanning {
                progress: lock.len(),
            };
            
            // Explicit drop to release lock between batches
            drop(lock);
        }
    }
    
    let duration_ms = start_time.elapsed().as_millis() as u64;
    
    Ok(IndexResult {
        files_added: total_added,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_creation() {
        let index = Arc::new(RwLock::new(FileIndex::new()));
        let indexer = Indexer::new(index);
        assert!(indexer.is_ok());
    }

    #[test]
    fn test_state_transitions() {
        let index = Arc::new(RwLock::new(FileIndex::new()));
        let indexer = Indexer::new(index).unwrap();
        
        match indexer.state() {
            IndexState::Idle => {},
            _ => panic!("Should start in Idle state"),
        }
    }
}
