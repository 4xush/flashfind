use eframe::egui;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use crate::index::FileIndex;
use crate::indexer::{Indexer, IndexState};
use crate::persistence::{load_index, save_index};
use crate::watcher::{get_default_directories, Watcher};

/// Main application state
pub struct FlashFindApp {
    index: Arc<RwLock<FileIndex>>,
    indexer: Indexer,
    watcher: Option<Watcher>,
    query: String,
    results: Vec<PathBuf>,
    search_time_ms: f64,
    last_error: Option<String>,
    show_settings: bool,
}

impl FlashFindApp {
    /// Create a new FlashFindApp instance
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize logging
        init_logging();
        
        info!("FlashFind starting up");
        
        // Setup UI styling
        setup_ui_style(&cc.egui_ctx);
        
        // Load or create index
        let index = match load_index() {
            Ok(idx) => {
                info!("Loaded existing index with {} files", idx.len());
                Arc::new(RwLock::new(idx))
            }
            Err(e) => {
                warn!("Failed to load index ({}), creating new one", e);
                Arc::new(RwLock::new(FileIndex::new()))
            }
        };
        
        // Create indexer
        let indexer = match Indexer::new(index.clone()) {
            Ok(idx) => idx,
            Err(e) => {
                error!("Failed to create indexer: {}", e);
                panic!("Cannot start without indexer");
            }
        };
        
        // Setup filesystem watcher
        let watcher = match Watcher::new(index.clone()) {
            Ok(mut w) => {
                let dirs = get_default_directories();
                match w.watch_directories(dirs) {
                    Ok(errors) => {
                        for err in errors {
                            warn!("Watcher error: {}", err);
                        }
                    }
                    Err(e) => error!("Failed to setup watchers: {}", e),
                }
                Some(w)
            }
            Err(e) => {
                warn!("Failed to create watcher ({}), real-time updates disabled", e);
                None
            }
        };
        
        // Start initial scan if index is empty
        let needs_scan = index.read().is_empty();
        if needs_scan {
            info!("Index is empty, starting initial scan");
            let dirs = get_default_directories();
            if let Err(e) = indexer.start_scan(dirs) {
                error!("Failed to start initial scan: {}", e);
            }
        }
        
        Self {
            index,
            indexer,
            watcher,
            query: String::new(),
            results: Vec::new(),
            search_time_ms: 0.0,
            last_error: None,
            show_settings: false,
        }
    }
    
    /// Perform a search
    fn do_search(&mut self) {
        let start = Instant::now();
        self.results = self.index.read().search(&self.query);
        self.search_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        debug!("Search completed in {:.2}ms", self.search_time_ms);
    }
    
    /// Handle manual save button
    fn handle_save(&mut self) {
        match save_index(&*self.index.read()) {
            Ok(()) => {
                info!("Manual save successful");
                self.last_error = None;
            }
            Err(e) => {
                error!("Manual save failed: {}", e);
                self.last_error = Some(e.user_message());
            }
        }
    }
    
    /// Handle re-index button
    fn handle_reindex(&mut self) {
        let dirs = get_default_directories();
        match self.indexer.start_scan(dirs) {
            Ok(()) => {
                info!("Re-indexing started");
                self.last_error = None;
            }
            Err(e) => {
                error!("Failed to start re-indexing: {}", e);
                self.last_error = Some(e.user_message());
            }
        }
    }
    
    /// Safely open a file
    fn open_file(&mut self, path: &Path) {
        if !path.exists() {
            self.last_error = Some(format!("File not found: {}", path.display()));
            return;
        }
        
        match open::that(path) {
            Ok(()) => debug!("Opened file: {}", path.display()),
            Err(e) => {
                error!("Failed to open file: {}", e);
                self.last_error = Some(format!("Cannot open file: {}", e));
            }
        }
    }
    
    /// Safely open a folder
    fn open_folder(&mut self, path: &Path) {
        match open::that(path) {
            Ok(()) => debug!("Opened folder: {}", path.display()),
            Err(e) => {
                error!("Failed to open folder: {}", e);
                self.last_error = Some(format!("Cannot open folder: {}", e));
            }
        }
    }
}

impl eframe::App for FlashFindApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let total_files = self.index.read().len();
        let state = self.indexer.state();
        let is_indexing = self.indexer.is_running();
        
        // Handle keyboard shortcuts
        let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let first_result = if !self.results.is_empty() {
            Some(self.results[0].clone())
        } else {
            None
        };
        
        if escape_pressed {
            self.query.clear();
            self.results.clear();
            self.last_error = None;
        }
        
        if enter_pressed {
            if let Some(path) = first_result {
                self.open_file(&path);
            }
        }
        
        // Header panel
        let mut should_save = false;
        let mut should_reindex = false;
        
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚡").size(20.0));
                    ui.heading("FlashFind");
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // State indicator
                        match &state {
                            IndexState::Scanning { progress } => {
                                ui.add(egui::Spinner::new().size(12.0));
                                ui.label(egui::RichText::new(format!("Indexing... {}", progress)).weak().small());
                            }
                            IndexState::Saving => {
                                ui.label(egui::RichText::new("Saving...").weak().small());
                            }
                            IndexState::Error { message } => {
                                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), message);
                            }
                            IndexState::Idle => {
                                ui.label(egui::RichText::new(format!("{} files", total_files)).weak().small());
                            }
                        }
                        
                        if ui.button("💾 Save").clicked() {
                            should_save = true;
                        }
                        
                        if ui.button("🔄 Re-index").clicked() {
                            should_reindex = true;
                        }
                        
                        if ui.button("⚙ Settings").clicked() {
                            self.show_settings = !self.show_settings;
                        }
                    });
                });
                
                ui.add_space(8.0);
                
                // Search box
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Type to search... (Enter to open, Esc to clear)")
                        .desired_width(f32::INFINITY)
                        .lock_focus(true),
                );
                
                if search.changed() {
                    self.do_search();
                }
                
                // Show search stats
                if !self.results.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} results in {:.2}ms",
                            self.results.len(),
                            self.search_time_ms
                        ))
                        .weak()
                        .small(),
                    );
                }
                
                // Show errors
                if let Some(err) = &self.last_error {
                    ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                }
            });
        
        // Handle button actions after UI
        if should_save {
            self.handle_save();
        }
        if should_reindex {
            self.handle_reindex();
        }
        
        // Settings window
        if self.show_settings {
            egui::Window::new("⚙ Settings")
                .open(&mut self.show_settings)
                .resizable(true)
                .default_width(400.0)
                .show(ctx, |ui| {
                    render_settings(ui, &self.index, &self.indexer, &self.watcher);
                });
        }
        
        // Main results panel
        let results_clone = self.results.clone();
        let mut action_queue: Vec<(PathBuf, ResultAction)> = Vec::new();
        
        egui::CentralPanel::default().show(ctx, |ui| {
            if results_clone.is_empty() && self.query.is_empty() {
                render_empty_state(ui, total_files);
            } else if results_clone.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("No results found").weak());
                });
            } else {
                render_results(ui, &results_clone, &mut action_queue);
            }
        });
        
        // Process actions after UI rendering
        for (path, action) in action_queue {
            match action {
                ResultAction::Open => self.open_file(&path),
                ResultAction::OpenFolder => {
                    if let Some(parent) = path.parent() {
                        self.open_folder(parent);
                    }
                }
                ResultAction::CopyPath => {},
            }
        }
        
        // Request repaint if indexing
        if is_indexing {
            ctx.request_repaint();
        }
    }
    
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        info!("FlashFind shutting down");
        
        // Save index on exit
        match save_index(&*self.index.read()) {
            Ok(()) => info!("Index saved on exit"),
            Err(e) => error!("Failed to save index on exit: {}", e),
        }
    }
}

/// Actions that can be performed on results
enum ResultAction {
    Open,
    OpenFolder,
    CopyPath,
}

/// Render settings window
fn render_settings(ui: &mut egui::Ui, index: &Arc<RwLock<FileIndex>>, indexer: &Indexer, watcher: &Option<Watcher>) {
    ui.heading("FlashFind Settings");
    ui.add_space(10.0);
    
    // Index statistics
    ui.group(|ui| {
        ui.label(egui::RichText::new("📊 Index Statistics").strong());
        ui.separator();
        
        let stats = index.read();
        let (insertions, duplicates, searches) = stats.stats();
        
        ui.horizontal(|ui| {
            ui.label("Total files:");
            ui.label(egui::RichText::new(format!("{}", stats.len())).strong());
        });
        ui.horizontal(|ui| {
            ui.label("Insertions:");
            ui.label(format!("{}", insertions));
        });
        ui.horizontal(|ui| {
            ui.label("Duplicates skipped:");
            ui.label(format!("{}", duplicates));
        });
        ui.horizontal(|ui| {
            ui.label("Searches performed:");
            ui.label(format!("{}", searches));
        });
        ui.horizontal(|ui| {
            ui.label("Index version:");
            ui.label(format!("v{}", stats.version()));
        });
    });
    
    ui.add_space(10.0);
    
    // Indexer state
    ui.group(|ui| {
        ui.label(egui::RichText::new("⚙️ Indexer Status").strong());
        ui.separator();
        
        match indexer.state() {
            IndexState::Idle => {
                ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "✓ Idle");
            }
            IndexState::Scanning { progress } => {
                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), format!("🔄 Scanning: {} files", progress));
            }
            IndexState::Saving => {
                ui.colored_label(egui::Color32::from_rgb(100, 200, 255), "💾 Saving...");
            }
            IndexState::Error { message } => {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), format!("❌ Error: {}", message));
            }
        }
    });
    
    ui.add_space(10.0);
    
    // Watched directories
    if let Some(w) = watcher {
        ui.group(|ui| {
            ui.label(egui::RichText::new("👁 Watched Directories").strong());
            ui.separator();
            
            let watched = w.watched_directories();
            if watched.is_empty() {
                ui.label(egui::RichText::new("No directories being watched").weak());
            } else {
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for dir in watched {
                            ui.label(format!("📁 {}", dir.display()));
                        }
                    });
            }
        });
    } else {
        ui.colored_label(egui::Color32::from_rgb(255, 150, 100), "⚠ File watcher disabled");
    }
    
    ui.add_space(10.0);
    
    // About
    ui.group(|ui| {
        ui.label(egui::RichText::new("ℹ About").strong());
        ui.separator();
        ui.label("FlashFind v1.0.0-phase1");
        ui.label("High-performance file search for Windows");
        ui.hyperlink_to("GitHub", "https://github.com");
    });
}

/// Render the header bar
/// Render empty state (no search query)
fn render_empty_state(ui: &mut egui::Ui, total_files: usize) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("⚡").size(64.0));
            ui.add_space(16.0);
            ui.heading("FlashFind");
            ui.add_space(8.0);
            ui.label(format!("{} files indexed", total_files));
            ui.add_space(16.0);
            ui.label(egui::RichText::new("Start typing to search...").weak());
        });
    });
}

/// Render search results with virtual scrolling
fn render_results(ui: &mut egui::Ui, results: &[PathBuf], action_queue: &mut Vec<(PathBuf, ResultAction)>) {
    let row_height = 30.0;
    
    egui::ScrollArea::vertical().show_rows(ui, row_height, results.len(), |ui, range| {
        for i in range {
            let path = &results[i];
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            
            // Use unique ID for each row based on full path and index
            ui.push_id(format!("result_{}", i), |ui| {
                let response = ui.horizontal(|ui| {
                    ui.set_height(row_height);
                    ui.label(get_file_icon(path));
                    
                    let link = ui.link(&filename);
                    if link.clicked() {
                        action_queue.push((path.clone(), ResultAction::Open));
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(
                                path.parent()
                                    .unwrap_or(Path::new(""))
                                    .to_string_lossy()
                            )
                            .weak()
                            .small(),
                        );
                    });
                }).response;
                
                // Context menu with unique ID
                response.context_menu(|ui| {
                    if ui.button("📂 Open Folder").clicked() {
                        action_queue.push((path.clone(), ResultAction::OpenFolder));
                        ui.close_menu();
                    }
                    if ui.button("📋 Copy Path").clicked() {
                        ui.output_mut(|o| o.copied_text = path.to_string_lossy().to_string());
                        action_queue.push((path.clone(), ResultAction::CopyPath));
                        ui.close_menu();
                    }
                });
            });
        }
    });
}

/// Get icon for file type
fn get_file_icon(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    
    match ext.as_str() {
        "pdf" => "📕",
        "docx" | "doc" | "txt" | "md" => "📄",
        "xlsx" | "xls" | "csv" => "📊",
        "pptx" | "ppt" => "📊",
        "exe" | "msi" => "⚙️",
        "jpg" | "jpeg" | "png" | "gif" | "bmp" => "🖼️",
        "zip" | "7z" | "rar" | "tar" | "gz" => "📦",
        "mp4" | "mkv" | "avi" | "mov" => "🎥",
        "mp3" | "wav" | "flac" | "m4a" => "🎵",
        "rs" | "py" | "js" | "ts" | "java" | "cpp" | "c" | "h" => "💻",
        "html" | "css" | "json" | "xml" => "🌐",
        _ => "📁",
    }
}

/// Setup UI styling
fn setup_ui_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.widgets.active.rounding = egui::Rounding::same(4.0);
    visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);
    visuals.window_rounding = egui::Rounding::same(8.0);
    ctx.set_visuals(visuals);
}

/// Initialize logging system
fn init_logging() {
    use tracing::Level;
    
    let log_path = match crate::persistence::get_log_path() {
        Ok(path) => path,
        Err(_) => {
            // Fallback: only show errors and warnings
            eprintln!("Failed to get log path");
            let _ = tracing_subscriber::fmt()
                .with_max_level(Level::WARN)
                .try_init();
            return;
        }
    };
    
    let file_appender = tracing_appender::rolling::daily(
        log_path.parent().unwrap_or(Path::new(".")),
        "flashfind.log",
    );
    
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // In debug builds, all logs go to file, only warnings/errors to console
    // In release builds, all logs go to file only (no console output)
    #[cfg(debug_assertions)]
    {
        let _ = tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_max_level(Level::DEBUG)
            .try_init();
        
        info!("Debug mode: Full logging to file, warnings to console");
    }
    
    #[cfg(not(debug_assertions))]
    {
        let _ = tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_max_level(Level::INFO)
            .try_init();
    }
    
    // Keep the file appender alive
    std::mem::forget(_guard);
}
