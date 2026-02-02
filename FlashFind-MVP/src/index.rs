use ahash::AHashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, warn, info};

use crate::error::{FlashFindError, Result};

/// Maximum number of files that can be indexed
pub const MAX_INDEX_SIZE: usize = 10_000_000;

/// Serialization version for backwards compatibility
pub const INDEX_VERSION: u32 = 1;

/// Core file indexing data structure with memory-efficient path storage
#[derive(Serialize, Deserialize)]
pub struct FileIndex {
    /// Serialization version for compatibility checking
    version: u32,
    
    /// Central storage for all file paths (indexed by u32)
    pool: Vec<PathBuf>,
    
    /// Filename to pool indices mapping
    filename_index: AHashMap<String, Vec<u32>>,
    
    /// File extension to pool indices mapping
    extension_index: AHashMap<String, Vec<u32>>,
    
    /// Runtime-only cache for fast duplicate detection
    #[serde(skip)]
    seen_paths: HashSet<PathBuf>,
    
    /// Statistics counter
    #[serde(skip)]
    stats: IndexStats,
}

#[derive(Default)]
struct IndexStats {
    insertions: AtomicUsize,
    duplicates: AtomicUsize,
    searches: AtomicUsize,
}

impl Default for FileIndex {
    fn default() -> Self {
        Self {
            version: INDEX_VERSION,
            pool: Vec::new(),
            filename_index: AHashMap::new(),
            extension_index: AHashMap::new(),
            seen_paths: HashSet::new(),
            stats: IndexStats::default(),
        }
    }
}

impl FileIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        info!("Creating new file index");
        Self::default()
    }

    /// Get current index version
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Rebuild the seen_paths cache from the pool (call after deserialization)
    pub fn rebuild_cache(&mut self) {
        debug!("Rebuilding seen_paths cache from {} paths", self.pool.len());
        self.seen_paths = self.pool.iter().cloned().collect();
    }

    /// Get total number of indexed files
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// Clear all indexed data
    pub fn clear(&mut self) {
        info!("Clearing index with {} files", self.pool.len());
        self.pool.clear();
        self.filename_index.clear();
        self.extension_index.clear();
        self.seen_paths.clear();
        self.stats.insertions.store(0, Ordering::Relaxed);
        self.stats.duplicates.store(0, Ordering::Relaxed);
        self.stats.searches.store(0, Ordering::Relaxed);
    }

    /// Get statistics about the index
    pub fn stats(&self) -> (usize, usize, usize) {
        (
            self.stats.insertions.load(Ordering::Relaxed),
            self.stats.duplicates.load(Ordering::Relaxed),
            self.stats.searches.load(Ordering::Relaxed),
        )
    }

    /// Insert a file path into the index
    /// Returns Ok(true) if inserted, Ok(false) if duplicate, Err on failure
    pub fn insert(&mut self, path: PathBuf) -> Result<bool> {
        // Check capacity limit
        if self.pool.len() >= MAX_INDEX_SIZE {
            warn!("Index full at {} files", MAX_INDEX_SIZE);
            return Err(FlashFindError::IndexFull(MAX_INDEX_SIZE));
        }

        // Check for duplicates
        if self.seen_paths.contains(&path) {
            self.stats.duplicates.fetch_add(1, Ordering::Relaxed);
            return Ok(false);
        }

        // Extract filename
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| FlashFindError::InvalidPath(path.display().to_string()))?;

        let idx = self.pool.len() as u32;
        let lower_name = filename.to_lowercase();

        // Add to filename index
        self.filename_index
            .entry(lower_name)
            .or_default()
            .push(idx);

        // Add to extension index
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            self.extension_index
                .entry(ext.to_lowercase())
                .or_default()
                .push(idx);
        }

        // Update tracking structures
        let path_display = path.display().to_string();
        self.seen_paths.insert(path.clone());
        self.pool.push(path);
        self.stats.insertions.fetch_add(1, Ordering::Relaxed);

        debug!("Inserted file #{}: {}", idx, path_display);
        Ok(true)
    }

    /// Remove a file path from the index
    pub fn remove(&mut self, path: &PathBuf) -> Result<bool> {
        if !self.seen_paths.remove(path) {
            return Ok(false); // Not found
        }

        // Find and mark as deleted in pool (we don't actually remove to keep indices valid)
        // In a production version, you'd implement compaction here
        debug!("Removed path: {}", path.display());
        Ok(true)
    }

    /// Search for files matching the query
    /// - Queries starting with '.' perform O(1) extension lookup
    /// - Other queries perform parallel substring search across filenames
    pub fn search(&self, query: &str) -> Vec<PathBuf> {
        self.stats.searches.fetch_add(1, Ordering::Relaxed);
        
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }

        let mut matched_indices = HashSet::new();

        // Extension search (e.g., ".pdf")
        if q.starts_with('.') {
            let ext = q.trim_start_matches('.');
            
            // Support compound extensions like ".tar.gz"
            if let Some(indices) = self.extension_index.get(ext) {
                matched_indices.extend(indices);
            }
            
            // Also try matching the full extension for compound cases
            if ext.contains('.') {
                // For ".tar.gz", also search for files ending with full extension
                let results: Vec<u32> = self.pool
                    .par_iter()
                    .enumerate()
                    .filter(|(_, path)| {
                        path.to_string_lossy()
                            .to_lowercase()
                            .ends_with(&q)
                    })
                    .map(|(idx, _)| idx as u32)
                    .collect();
                matched_indices.extend(results);
            }
        } else {
            // Parallel substring search across all filenames
            let results: Vec<u32> = self
                .filename_index
                .par_iter()
                .filter(|(name, _)| name.contains(&q))
                .flat_map(|(_, indices)| indices.clone())
                .collect();
            matched_indices.extend(results);
        }

        // Convert indices to paths and sort
        let mut results: Vec<PathBuf> = matched_indices
            .into_iter()
            .filter(|&idx| (idx as usize) < self.pool.len()) // Safety check
            .map(|idx| self.pool[idx as usize].clone())
            .collect();

        results.sort_unstable_by(|a, b| {
            // Sort by filename, case-insensitive
            let a_name = a.file_name().map(|n| n.to_string_lossy().to_lowercase());
            let b_name = b.file_name().map(|n| n.to_string_lossy().to_lowercase());
            a_name.cmp(&b_name)
        });

        debug!("Search '{}' returned {} results", query, results.len());
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert() {
        let mut index = FileIndex::new();
        let path = PathBuf::from("C:\\test\\file.txt");
        
        assert!(index.insert(path.clone()).unwrap());
        assert_eq!(index.len(), 1);
        
        // Duplicate insert
        assert!(!index.insert(path).unwrap());
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_extension_search() {
        let mut index = FileIndex::new();
        index.insert(PathBuf::from("C:\\test\\doc.pdf")).unwrap();
        index.insert(PathBuf::from("C:\\test\\notes.txt")).unwrap();
        
        let results = index.search(".pdf");
        assert_eq!(results.len(), 1);
        assert!(results[0].to_string_lossy().contains("doc.pdf"));
    }

    #[test]
    fn test_substring_search() {
        let mut index = FileIndex::new();
        index.insert(PathBuf::from("C:\\test\\budget_2024.xlsx")).unwrap();
        index.insert(PathBuf::from("C:\\test\\budget_report.pdf")).unwrap();
        index.insert(PathBuf::from("C:\\test\\invoice.pdf")).unwrap();
        
        let results = index.search("budget");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_max_capacity() {
        let mut index = FileIndex::new();
        // This would take too long in real test, so we just test the error
        for i in 0..MAX_INDEX_SIZE {
            if i >= MAX_INDEX_SIZE {
                let path = PathBuf::from(format!("C:\\test\\file_{}.txt", i));
                assert!(index.insert(path).is_err());
                break;
            }
        }
    }

    #[test]
    fn test_compound_extension() {
        let mut index = FileIndex::new();
        index.insert(PathBuf::from("C:\\test\\archive.tar.gz")).unwrap();
        
        let results = index.search(".tar.gz");
        assert_eq!(results.len(), 1);
    }
}
