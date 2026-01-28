use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use walkdir::WalkDir;
use open::that;

#[derive(Default)]
struct FileIndex {
    files: HashMap<String, Vec<PathBuf>>,
}

impl FileIndex {
    fn insert(&mut self, path: PathBuf) {
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            let key = filename.to_lowercase();
            self.files.entry(key).or_insert_with(Vec::new).push(path);
        }
    }
    
    fn search(&self, query: &str) -> Vec<PathBuf> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        
        for (filename, paths) in &self.files {
            if filename.contains(&query_lower) {
                results.extend(paths.iter().cloned());
            }
        }
        
        results.sort();
        results.truncate(100);
        results
    }
    
    fn len(&self) -> usize {
        self.files.values().map(|v| v.len()).sum()
    }
}

struct FlashFindApp {
    index: Arc<RwLock<FileIndex>>,
    query: String,
    results: Vec<String>,
    search_time_ms: f64,
}

impl FlashFindApp {
    fn new() -> Self {
        let index = Arc::new(RwLock::new(FileIndex::default()));
        
        // Start indexing in background
        let index_clone = index.clone();
        std::thread::spawn(move || {
            println!("Indexing files...");
            
            // Index current directory and subdirectories
            for entry in WalkDir::new(".")
                .max_depth(3)  // Limit depth for performance
                .into_iter()
                .filter_map(|e| e.ok()) 
            {
                if entry.file_type().is_file() {
                    index_clone.write().unwrap().insert(entry.path().to_path_buf());
                }
            }
            
            let count = index_clone.read().unwrap().len();
            println!("Indexing complete! Indexed {} files.", count);
        });
        
        Self {
            index,
            query: String::new(),
            results: Vec::new(),
            search_time_ms: 0.0,
        }
    }
}

impl eframe::App for FlashFindApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("⚡ FlashFind MVP");
            
            // Search box
            let response = ui.text_edit_singleline(&mut self.query);
            
            if response.changed() {
                let start = std::time::Instant::now();
                let index = self.index.read().unwrap();
                let path_results = index.search(&self.query);
                let elapsed = start.elapsed();
                self.search_time_ms = elapsed.as_secs_f64() * 1000.0;
                
                // Convert PathBuf to strings
                self.results = path_results
                    .iter()
                    .map(|p: &PathBuf| p.to_string_lossy().to_string())
                    .collect();
            }
            
            ui.horizontal(|ui| {
                ui.label("Search time:");
                ui.label(format!("{:.1} ms", self.search_time_ms));
                ui.label(" | ");
                ui.label("Files indexed:");
                ui.label(self.index.read().unwrap().len().to_string());
            });
            
            ui.separator();
            
            ui.label(format!("Results: {}", self.results.len()));
            
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for (i, result) in self.results.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}. {}", i + 1, result));
                            
                            if ui.small_button("📋").clicked() {
                                // Copy to clipboard
                                ui.output_mut(|o| o.copied_text = result.clone());
                            }
                            
                            if ui.small_button("📂").clicked() {
                                // Open file
                                if let Err(e) = that(result) {
                                    eprintln!("Failed to open file: {}", e);
                                }
                            }
                        });
                    }
                });
        });
        
        // Request repaint for real-time updates
        ctx.request_repaint();
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("FlashFind MVP - Fast File Search"),
        ..Default::default()
    };
    
    eframe::run_native(
        "FlashFind MVP",
        options,
        Box::new(|_cc| Box::new(FlashFindApp::new())),
    )
}