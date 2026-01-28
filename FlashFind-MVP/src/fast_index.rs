use memmap2::Mmap;
use serde::{Serialize, Deserialize};
use std::fs::{File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct CompactIndex {
    pub filenames: Vec<String>,
    pub paths: Vec<String>,
    pub filename_to_paths: Vec<Vec<usize>>,
}

impl CompactIndex {
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let serialized = bincode::serialize(self).unwrap();
        let mut file = File::create(path)?;
        file.write_all(&serialized)?;
        Ok(())
    }
    
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let index: CompactIndex = bincode::deserialize(&mmap).unwrap();
        Ok(index)
    }
}