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

// --- PERSISTENCE & ENGINE ---

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
            // FIXED: Using Rayon here to scan keys in parallel, fixing the "unused" warning
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

// --- APP LOGIC ---

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
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let (mut loaded_index, needs_initial_scan) = Self::load_index();
        loaded_index.rebuild_cache();
        
        let index = Arc::new(RwLock::new(loaded_index));
        let is_indexing = Arc::new(RwLock::new(false));

        let index_for_watcher = index.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let mut lock = index_for_watcher.write();
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            if path.is_file() && !is_excluded(&path) {
                                lock.insert(path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }).expect("Failed to start watcher");

        let dirs = get_default_directories();
        for dir in &dirs {
            let _ = watcher.watch(dir, RecursiveMode::Recursive);
        }

        let app = Self {
            index,
            is_indexing,
            query: String::new(),
            results: Vec::new(),
            search_time_ms: 0.0,
            _watcher: watcher,
        };

        if needs_initial_scan {
            app.start_indexing(dirs);
        }

        app
    }

    fn get_cache_path() -> PathBuf {
        let mut path = PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into()));
        path.push("FlashFind");
        let _ = std::fs::create_dir_all(&path);
        path.push("index.bin");
        path
    }

    fn load_index() -> (FileIndex, bool) {
        let path = Self::get_cache_path();
        if let Ok(data) = std::fs::read(&path) {
            if let Ok(index) = bincode::deserialize::<FileIndex>(&data) {
                return (index, false);
            }
        }
        (FileIndex::default(), true)
    }

    fn save_index(&self) {
        let path = Self::get_cache_path();
        let lock = self.index.read();
        if let Ok(data) = bincode::serialize(&*lock) {
            let _ = std::fs::write(path, data);
        }
    }

    fn start_indexing(&self, dirs: Vec<PathBuf>) {
        let index_ptr = self.index.clone();
        let status_ptr = self.is_indexing.clone();

        std::thread::spawn(move || {
            *status_ptr.write() = true;
            for dir in dirs {
                let entries: Vec<PathBuf> = WalkDir::new(dir)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file() && !is_excluded(e.path()))
                    .map(|e| e.into_path())
                    .collect();

                let mut lock = index_ptr.write();
                for path in entries {
                    lock.insert(path);
                }
            }
            *status_ptr.write() = false;
        });
    }
}

impl eframe::App for FlashFindApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (total_files, indexing) = {
            let lock = self.index.read();
            (lock.pool.len(), *self.is_indexing.read())
        };

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("⚡ FlashFind");
                ui.label(egui::RichText::new(format!("{} files indexed", total_files)).weak());
                if indexing { ui.spinner(); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("💾 Save Index").clicked() {
                        self.save_index();
                    }
                });
            });
            ui.add_space(8.0);
            let search = ui.add(egui::TextEdit::singleline(&mut self.query)
                .hint_text("Search by filename or .extension...")
                .desired_width(f32::INFINITY));
            
            if search.changed() {
                let start = Instant::now();
                self.results = self.index.read().search(&self.query);
                self.search_time_ms = start.elapsed().as_secs_f64() * 1000.0;
            }
            ui.add_space(8.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() && self.query.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Start typing to search...").weak());
                });
            } else {
                let row_height = 25.0;
                egui::ScrollArea::vertical().show_rows(ui, row_height, self.results.len(), |ui, range| {
                    for i in range {
                        let path = &self.results[i];
                        ui.horizontal(|ui| {
                            ui.set_height(row_height);
                            ui.label(get_icon(path));
                            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
                            if ui.link(filename).on_hover_text(path.to_string_lossy()).clicked() {
                                let _ = open::that(path);
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(path.parent().map(|p| p.to_string_lossy()).unwrap_or_default()).weak().small());
                            });
                        });
                    }
                });
            }
        });

        if indexing { ctx.request_repaint(); }
    }
}

// --- HELPERS ---

fn get_icon(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
    match ext.as_str() {
        "pdf" => "📕",
        "docx" | "doc" | "txt" | "md" => "📄",
        "exe" | "msi" => "⚙️",
        "jpg" | "png" | "gif" | "bmp" => "🖼️",
        "zip" | "7z" | "rar" => "📦",
        "mp4" | "mkv" | "mov" => "🎥",
        "mp3" | "wav" | "flac" => "🎵",
        _ => "📁",
    }
}

fn is_excluded(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    s.contains("$recycle.bin") || 
    s.contains("appdata\\local\\temp") || 
    s.contains("node_modules") || 
    s.contains(".git") ||
    s.contains("target\\") // Exclude rust build artifacts
}

fn get_default_directories() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        for d in ["Documents", "Downloads", "Desktop", "Pictures", "Videos", "Music"] {
            let p = home.join(d);
            if p.exists() { dirs.push(p); }
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