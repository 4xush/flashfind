#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ahash::AHashMap;
use eframe::egui;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

// --- ENGINE (Optimized) ---

#[derive(Default)]
struct FileIndex {
    pool: Vec<PathBuf>,
    filename_index: AHashMap<String, Vec<u32>>,
    extension_index: AHashMap<String, Vec<u32>>,
    seen_paths: HashSet<PathBuf>,
}

impl FileIndex {
    fn insert(&mut self, path: PathBuf) {
        if self.seen_paths.contains(&path) { return; }
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            let idx = self.pool.len() as u32;
            let lower_name = filename.to_lowercase();
            self.filename_index.entry(lower_name).or_default().push(idx);
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                self.extension_index.entry(ext.to_lowercase()).or_default().push(idx);
            }
            self.seen_paths.insert(path.clone());
            self.pool.push(path);
        }
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
            // Use Rayon to parallelize substring search if the index is large
            let keys: Vec<_> = self.filename_index.keys().collect();
            let matches: Vec<String> = keys.into_par_iter()
                .filter(|name| name.contains(&q))
                .cloned()
                .collect();

            for name in matches {
                if let Some(indices) = self.filename_index.get(&name) {
                    matched_indices.extend(indices);
                }
            }
        }

        let mut results: Vec<PathBuf> = matched_indices
            .into_iter()
            .map(|&idx| self.pool[idx as usize].clone())
            .collect();

        results.sort_unstable_by_key(|p| p.file_name().map(|n| n.to_owned()));
        results
    }
}

// --- UI / APP ---

struct FlashFindApp {
    index: Arc<RwLock<FileIndex>>,
    is_indexing: Arc<RwLock<bool>>,
    query: String,
    results: Vec<PathBuf>,
    search_time_ms: f64,
}

impl FlashFindApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Professional visual tweak: Set dark mode by default
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let app = Self {
            index: Arc::new(RwLock::new(FileIndex::default())),
            is_indexing: Arc::new(RwLock::new(false)),
            query: String::new(),
            results: Vec::new(),
            search_time_ms: 0.0,
        };

        app.start_indexing();
        app
    }

    fn start_indexing(&self) {
        let index_ptr = self.index.clone();
        let status_ptr = self.is_indexing.clone();
        std::thread::spawn(move || {
            *status_ptr.write() = true;
            let dirs = get_default_directories();
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

        egui::TopBottomPanel::top("header_panel").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading("⚡ FlashFind");
                    ui.label(egui::RichText::new(format!("{} files", total_files)).weak());
                    if indexing {
                        ui.spinner();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.search_time_ms > 0.0 {
                            ui.label(format!("{:.2}ms", self.search_time_ms));
                        }
                    });
                });
                
                ui.add_space(4.0);
                
                let search_box = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search files... (e.g. .pdf, budget, report)")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(8.0, 8.0))
                );

                if search_box.changed() {
                    let start = Instant::now();
                    self.results = self.index.read().search(&self.query);
                    self.search_time_ms = start.elapsed().as_secs_f64() * 1000.0;
                }
                ui.add_space(8.0);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Start typing to find files").weak().size(16.0));
                });
            } else {
                let row_height = 24.0;
                let total_rows = self.results.len();

                // VIRTUAL SCROLLING: This is what makes it professional.
                // It only renders the ~30 rows visible on screen, not all 50,000.
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show_rows(ui, row_height, total_rows, |ui, row_range| {
                        for i in row_range {
                            let path = &self.results[i];
                            ui.horizontal(|ui| {
                                ui.set_height(row_height);
                                
                                // File Icon Placeholder
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

        if indexing {
            ctx.request_repaint();
        }
    }
}

// --- HELPERS ---

fn get_icon(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase().as_str() {
        "pdf" => "📕",
        "docx" | "doc" | "txt" | "md" => "📄",
        "exe" | "msi" => "⚙️",
        "jpg" | "png" | "gif" => "🖼️",
        "zip" | "7z" | "rar" => "📦",
        _ => "📁",
    }
}

fn is_excluded(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    s.contains("$recycle.bin") || s.contains("appdata\\local\\temp") || s.contains("node_modules") || s.contains(".git")
}

fn get_default_directories() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        // Search more broadly for production
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
    eframe::run_native(
        "FlashFind",
        options,
        Box::new(|cc| Box::new(FlashFindApp::new(cc))),
    )
}