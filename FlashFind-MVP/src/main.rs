#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ahash::AHashMap;
use eframe::egui;
use notify::{Watcher, RecursiveMode, EventKind};
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

// --- ENGINE & PERSISTENCE ---

#[derive(Default, Serialize, Deserialize)]
struct FileIndex {
    pool: Vec<PathBuf>,
    filename_index: AHashMap<String, Vec<u32>>,
    extension_index: AHashMap<String, Vec<u32>>,
    #[serde(skip)] 
    seen_paths: HashSet<PathBuf>,
}

impl FileIndex {
    fn rebuild_cache(&mut self) {
        self.seen_paths = self.pool.iter().cloned().collect();
    }

    fn insert(&mut self, path: PathBuf) -> bool {
        if self.seen_paths.contains(&path) { return false; }
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            let idx = self.pool.len() as u32;
            let lower_name = filename.to_lowercase();
            self.filename_index.entry(lower_name).or_default().push(idx);
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                self.extension_index.entry(ext.to_lowercase()).or_default().push(idx);
            }
            self.seen_paths.insert(path.clone());
            self.pool.push(path);
            return true;
        }
        false
    }

    fn search(&self, query: &str) -> Vec<PathBuf> {
        let q = query.to_lowercase();
        if q.is_empty() { return vec![]; }
        let mut matched_indices = HashSet::new();

        if q.starts_with('.') {
            let ext = q.trim_start_matches('.');
            if let Some(indices) = self.extension_index.get(ext) {
                matched_indices.extend(indices);
            }
        } else {
            let results: Vec<u32> = self.filename_index.par_iter()
                .filter(|(name, _)| name.contains(&q))
                .flat_map(|(_, indices)| indices.clone())
                .collect();
            matched_indices.extend(results);
        }

        let mut results: Vec<PathBuf> = matched_indices.into_iter()
            .map(|idx| self.pool[idx as usize].clone())
            .collect();
            
        results.sort_unstable_by_key(|p| p.file_name().map(|n| n.to_owned()));
        results
    }
}

// --- APP CORE ---

struct FlashFindApp {
    index: Arc<RwLock<FileIndex>>,
    is_indexing: Arc<RwLock<bool>>,
    query: String,
    results: Vec<PathBuf>,
    search_time_ms: f64,
    _watcher: notify::RecommendedWatcher,
}

impl FlashFindApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // High-end UI styling
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.active.rounding = egui::Rounding::same(4.0);
        visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);
        cc.egui_ctx.set_visuals(visuals);

        let (mut loaded_index, needs_initial_scan) = Self::load_index();
        loaded_index.rebuild_cache();
        
        let index = Arc::new(RwLock::new(loaded_index));
        let is_indexing = Arc::new(RwLock::new(false));

        // Watcher setup
        let index_for_watcher = index.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let mut lock = index_for_watcher.write();
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            if path.is_file() && !is_excluded(&path) { lock.insert(path); }
                        }
                    }
                    _ => {}
                }
            }
        }).expect("Failed watcher");

        let dirs = get_default_directories();
        for dir in &dirs { let _ = watcher.watch(dir, RecursiveMode::Recursive); }

        let app = Self {
            index,
            is_indexing,
            query: String::new(),
            results: Vec::new(),
            search_time_ms: 0.0,
            _watcher: watcher,
        };

        if needs_initial_scan { app.start_indexing(dirs); }
        app
    }

    fn load_index() -> (FileIndex, bool) {
        let path = get_cache_path();
        if let Ok(data) = std::fs::read(&path) {
            if let Ok(index) = bincode::deserialize::<FileIndex>(&data) { return (index, false); }
        }
        (FileIndex::default(), true)
    }

    // RESTORED: Manual save function for UI button and auto-save
    fn save_index(&self) {
        let path = get_cache_path();
        let lock = self.index.read();
        if let Ok(data) = bincode::serialize(&*lock) {
            let _ = std::fs::write(path, data);
        }
    }

    fn start_indexing(&self, dirs: Vec<PathBuf>) {
        let index_ptr = self.index.clone();
        let status_ptr = self.is_indexing.clone();
        let app_handle = self.index.clone(); // For auto-save

        std::thread::spawn(move || {
            *status_ptr.write() = true;
            for dir in dirs {
                let entries: Vec<PathBuf> = WalkDir::new(dir).follow_links(false).into_iter().filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file() && !is_excluded(e.path())).map(|e| e.into_path()).collect();
                let mut lock = index_ptr.write();
                for path in entries { lock.insert(path); }
            }
            *status_ptr.write() = false;

            // RESTORED: Auto-save to disk as soon as indexing completes
            let path = get_cache_path();
            let lock = app_handle.read();
            if let Ok(data) = bincode::serialize(&*lock) {
                let _ = std::fs::write(path, data);
            }
        });
    }
}

impl eframe::App for FlashFindApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (total_files, indexing) = {
            let lock = self.index.read();
            (lock.pool.len(), *self.is_indexing.read())
        };

        // KEYBOARD SHORTCUTS
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.query.clear();
            self.results.clear();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !self.results.is_empty() {
            let _ = open::that(&self.results[0]);
        }

        egui::TopBottomPanel::top("header").frame(egui::Frame::none().inner_margin(8.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚡").size(20.0));
                ui.heading("FlashFind");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if indexing { ui.add(egui::Spinner::new().size(12.0)); }
                    
                    // RESTORED: The explicit Save Button (though auto-save handles most of it)
                    if ui.button("💾 Save Index").clicked() {
                        self.save_index();
                    }
                    
                    ui.label(egui::RichText::new(format!("{} files", total_files)).weak().small());
                    if ui.button("🔄 Re-index").clicked() { self.start_indexing(get_default_directories()); }
                });
            });
            ui.add_space(8.0);
            
            let search = ui.add(egui::TextEdit::singleline(&mut self.query)
                .hint_text("Type to search... (Enter to open top result, Esc to clear)")
                .desired_width(f32::INFINITY)
                .lock_focus(true));
            
            if search.changed() {
                let start = Instant::now();
                self.results = self.index.read().search(&self.query);
                self.search_time_ms = start.elapsed().as_secs_f64() * 1000.0;
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("No results").weak());
                });
            } else {
                let row_height = 30.0;
                egui::ScrollArea::vertical().show_rows(ui, row_height, self.results.len(), |ui, range| {
                    for i in range {
                        let path = &self.results[i];
                        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        
                        let response = ui.horizontal(|ui| {
                            ui.set_height(row_height);
                            ui.label(get_icon(path));
                            let link = ui.link(&filename);
                            if link.clicked() { let _ = open::that(path); }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(path.parent().unwrap_or(Path::new("")).to_string_lossy()).weak().small());
                            });
                        }).response;

                        // CONTEXT MENU
                        response.context_menu(|ui| {
                            if ui.button("📂 Open Folder").clicked() {
                                if let Some(parent) = path.parent() { let _ = open::that(parent); }
                                ui.close_menu();
                            }
                            if ui.button("📋 Copy Path").clicked() {
                                ui.output_mut(|o| o.copied_text = path.to_string_lossy().to_string());
                                ui.close_menu();
                            }
                        });
                    }
                });
            }
        });

        if indexing { ctx.request_repaint(); }
    }
}

// --- SYSTEM HELPERS ---

fn get_cache_path() -> PathBuf {
    let mut path = PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into()));
    path.push("FlashFind");
    let _ = std::fs::create_dir_all(&path);
    path.push("index.bin");
    path
}

fn get_icon(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
    match ext.as_str() {
        "pdf" => "📕", "docx" | "doc" | "txt" | "md" => "📄",
        "exe" | "msi" => "⚙️", "jpg" | "png" | "gif" => "🖼️",
        "zip" | "7z" | "rar" => "📦", "mp4" | "mkv" => "🎥",
        "mp3" | "wav" => "🎵", _ => "📁",
    }
}

fn is_excluded(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    s.contains("$recycle.bin") || s.contains("appdata") || s.contains("node_modules") || s.contains(".git")
}

fn get_default_directories() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        for d in ["Documents", "Downloads", "Desktop", "Pictures", "Videos"] {
            let p = home.join(d); if p.exists() { dirs.push(p); }
        }
    }
    dirs
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_title("FlashFind"),
        ..Default::default()
    };
    eframe::run_native("FlashFind", options, Box::new(|cc| Box::new(FlashFindApp::new(cc))))
}