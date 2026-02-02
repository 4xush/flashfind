use eframe::egui;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::config::{Config, Theme};
use crate::index::FileIndex;
use crate::indexer::{Indexer, IndexState};
use crate::persistence::{load_index, save_index};
use crate::watcher::{get_default_directories, Watcher};

/// File type filter options
#[derive(Debug, Clone, Copy, PartialEq)]
enum FileTypeFilter {
    All,
    Documents,
    Images,
    Videos,
    Audio,
    Code,
    Archives,
}

impl FileTypeFilter {
    fn matches(&self, path: &Path) -> bool {
        if matches!(self, FileTypeFilter::All) {
            return true;
        }
        
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());
        
        match ext.as_deref() {
            Some(e) => match self {
                FileTypeFilter::Documents => matches!(e, "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "md"),
                FileTypeFilter::Images => matches!(e, "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico"),
                FileTypeFilter::Videos => matches!(e, "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm"),
                FileTypeFilter::Audio => matches!(e, "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma"),
                FileTypeFilter::Code => matches!(e, "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "h" | "cs" | "go" | "rb" | "php" | "html" | "css" | "json" | "xml" | "yaml" | "toml"),
                FileTypeFilter::Archives => matches!(e, "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz"),
                FileTypeFilter::All => true,
            },
            None => false,
        }
    }
    
    fn label(&self) -> &'static str {
        match self {
            FileTypeFilter::All => "All Files",
            FileTypeFilter::Documents => "Documents",
            FileTypeFilter::Images => "Images",
            FileTypeFilter::Videos => "Videos",
            FileTypeFilter::Audio => "Audio",
            FileTypeFilter::Code => "Code",
            FileTypeFilter::Archives => "Archives",
        }
    }
}

/// Main application state
pub struct FlashFindApp {
    index: Arc<RwLock<FileIndex>>,
    indexer: Indexer,
    watcher: Option<Watcher>,
    config: Config,
    query: String,
    file_type_filter: FileTypeFilter,
    results: Vec<PathBuf>,
    search_time_ms: f64,
    last_error: Option<String>,
    show_settings: bool,
    settings_tab: SettingsTab,
    last_save: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsTab {
    Configuration,
    Drives,
    Statistics,
    Status,
    Directories,
    About,
}

impl FlashFindApp {
    /// Create a new FlashFindApp instance
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize logging
        init_logging();
        
        info!("FlashFind starting up");
        
        // Load configuration
        let config = Config::load().unwrap_or_else(|e| {
            warn!("Failed to load config ({}), using defaults", e);
            Config::default()
        });
        
        // Setup UI styling with theme
        setup_ui_style(&cc.egui_ctx, config.theme);
        
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
            config,
            query: String::new(),
            file_type_filter: FileTypeFilter::All,
            results: Vec::new(),
            search_time_ms: 0.0,
            last_error: None,
            show_settings: false,
            settings_tab: SettingsTab::Configuration,
            last_save: Instant::now(),
        }
    }
    
    /// Perform a search
    fn do_search(&mut self) {
        let start = Instant::now();
        let all_results = self.index.read().search(&self.query);
        
        // Apply file type filter
        self.results = if matches!(self.file_type_filter, FileTypeFilter::All) {
            all_results
        } else {
            all_results.into_iter()
                .filter(|path| self.file_type_filter.matches(path))
                .collect()
        };
        
        self.search_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        debug!("Search completed in {:.2}ms, {} results after filter", self.search_time_ms, self.results.len());
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
        // Sanitize path
        if !Self::is_safe_path(path) {
            self.last_error = Some(format!("Unsafe path: {}", path.display()));
            warn!("Attempted to open unsafe path: {}", path.display());
            return;
        }
        
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
        // Sanitize path
        if !Self::is_safe_path(path) {
            self.last_error = Some(format!("Unsafe path: {}", path.display()));
            warn!("Attempted to open unsafe path: {}", path.display());
            return;
        }
        
        match open::that(path) {
            Ok(()) => debug!("Opened folder: {}", path.display()),
            Err(e) => {
                error!("Failed to open folder: {}", e);
                self.last_error = Some(format!("Cannot open folder: {}", e));
            }
        }
    }
    
    /// Export search results to CSV file
    fn export_to_csv(&mut self) {
        use std::fs::File;
        use std::io::Write;
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let filename = format!("flashfind_export_{}.csv", timestamp);
        let export_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(&filename);
        
        match File::create(&export_path) {
            Ok(mut file) => {
                // Write CSV header
                if let Err(e) = writeln!(file, "Path,Filename,Extension,Size") {
                    self.last_error = Some(format!("Failed to write CSV: {}", e));
                    return;
                }
                
                // Write each result
                for path in &self.results {
                    let filename = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("N/A");
                    
                    let extension = path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("N/A");
                    
                    let size = std::fs::metadata(path)
                        .ok()
                        .map(|m| m.len())
                        .unwrap_or(0);
                    
                    let path_str = path.to_string_lossy();
                    
                    if let Err(e) = writeln!(file, "\"{}\",\"{}\",{},{}", path_str, filename, extension, size) {
                        warn!("Failed to write row: {}", e);
                    }
                }
                
                info!("Exported {} results to {}", self.results.len(), export_path.display());
                self.last_error = Some(format!("✓ Exported to {}", filename));
                
                // Open the folder containing the CSV
                if let Some(parent) = export_path.parent() {
                    let _ = open::that(parent);
                }
            }
            Err(e) => {
                error!("Failed to create CSV file: {}", e);
                self.last_error = Some(format!("Failed to export: {}", e));
            }
        }
    }
    
    /// Validate path is safe to open (no command injection, symlink attacks)
    fn is_safe_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        // Reject paths with suspicious characters
        if path_str.contains('&') || path_str.contains('|') || path_str.contains(';') {
            return false;
        }
        
        // Reject UNC paths that could be malicious
        if path_str.starts_with("\\\\") {
            return false;
        }
        
        // Path must be absolute
        if !path.is_absolute() {
            return false;
        }
        
        true
    }
    
    /// Render settings window
    fn render_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Configuration, "⚙️ Configuration");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Drives, "💾 Drives");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Statistics, "📊 Statistics");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Status, "⚙️ Status");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Directories, "👁 Directories");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::About, "ℹ About");
        });
        
        ui.separator();
        ui.add_space(10.0);
        
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                match self.settings_tab {
                    SettingsTab::Configuration => {
                        ui.heading("Configuration");
                        ui.add_space(10.0);
                        
                        // Theme selector
                        ui.horizontal(|ui| {
                            ui.label("Theme:");
                            let mut changed = false;
                            changed |= ui.selectable_value(&mut self.config.theme, Theme::Dark, "Dark").changed();
                            changed |= ui.selectable_value(&mut self.config.theme, Theme::Light, "Light").changed();
                            changed |= ui.selectable_value(&mut self.config.theme, Theme::System, "System").changed();
                            
                            if changed {
                                setup_ui_style(ctx, self.config.theme);
                                if let Err(e) = self.config.save() {
                                    warn!("Failed to save config: {}", e);
                                }
                            }
                        });
                        
                        ui.add_space(10.0);
                        
                        // Auto-save interval
                        ui.horizontal(|ui| {
                            ui.label("Auto-save interval:");
                            let mut minutes = (self.config.auto_save_interval / 60) as i32;
                            if ui.add(egui::Slider::new(&mut minutes, 0..=60).suffix(" min")).changed() {
                                self.config.auto_save_interval = (minutes as u64) * 60;
                                if let Err(e) = self.config.save() {
                                    warn!("Failed to save config: {}", e);
                                }
                            }
                        });
                        ui.label(egui::RichText::new("(0 = disabled)").weak().small());
                    }
                    
                    SettingsTab::Drives => {
                        ui.heading("Drive Selection");
                        ui.add_space(10.0);
                        
                        ui.label(egui::RichText::new("Select which drives to index:").weak());
                        ui.add_space(10.0);
                        
                        let available_drives = crate::watcher::get_available_drives();
                        
                        for drive in &available_drives {
                            let mut is_enabled = self.config.enabled_drives.contains(drive);
                            let drive_label = if *drive == 'C' {
                                format!("{}: (User folders: Documents, Downloads, Desktop, etc.)", drive)
                            } else {
                                format!("{}: (Coming soon)", drive)
                            };
                            
                            // Only C drive is functional for now
                            if *drive == 'C' {
                                if ui.checkbox(&mut is_enabled, drive_label).changed() {
                                    if is_enabled {
                                        if !self.config.enabled_drives.contains(drive) {
                                            self.config.enabled_drives.push(*drive);
                                        }
                                    } else {
                                        self.config.enabled_drives.retain(|d| d != drive);
                                    }
                                }
                            } else {
                                // Disabled checkbox for non-C drives
                                ui.add_enabled(false, egui::Checkbox::new(&mut false, drive_label));
                            }
                        }
                        
                        ui.add_space(10.0);
                        
                        if !self.config.enabled_drives.is_empty() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Selected: {}",
                                    self.config.enabled_drives.iter().collect::<String>()
                                ))
                                .weak()
                                .small()
                            );
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 150, 100),
                                "⚠ At least one drive must be selected"
                            );
                        }
                        
                        ui.add_space(10.0);
                        
                        if ui.button("🔄 Apply & Re-index").on_hover_text("Save drive selection and rebuild index").clicked() {
                            if !self.config.enabled_drives.is_empty() {
                                if let Err(e) = self.config.save() {
                                    warn!("Failed to save config: {}", e);
                                    self.last_error = Some(format!("Failed to save config: {}", e));
                                } else {
                                    // Clear existing index before re-indexing with new drive selection
                                    self.index.write().clear();
                                    
                                    // Trigger re-indexing
                                    let dirs = crate::watcher::get_directories_for_drives(&self.config.enabled_drives);
                                    if let Err(e) = self.indexer.start_scan(dirs.clone()) {
                                        error!("Failed to start re-indexing: {}", e);
                                        self.last_error = Some(e.user_message());
                                    } else {
                                        // Update watcher
                                        if let Some(ref mut watcher) = self.watcher {
                                            match watcher.watch_directories(dirs) {
                                                Ok(errors) => {
                                                    for err in errors {
                                                        warn!("Watcher error: {}", err);
                                                    }
                                                }
                                                Err(e) => error!("Failed to setup watchers: {}", e),
                                            }
                                        }
                                        info!("Re-indexing started for drives: {:?}", self.config.enabled_drives);
                                    }
                                }
                            } else {
                                self.last_error = Some("Please select at least one drive".to_string());
                            }
                        }
                        
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("ℹ Changes require clicking Apply to take effect")
                            .weak()
                            .small()
                        );
                    }
                    
                    SettingsTab::Statistics => {
                        ui.heading("Index Statistics");
                        ui.add_space(10.0);
                        
                        let stats = self.index.read();
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
                    }
                    
                    SettingsTab::Status => {
                        ui.heading("Indexer Status");
                        ui.add_space(10.0);
                        
                        match self.indexer.state() {
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
                    }
                    
                    SettingsTab::Directories => {
                        ui.heading("Watched Directories");
                        ui.add_space(10.0);
                        
                        if let Some(w) = &self.watcher {
                            let watched = w.watched_directories();
                            if watched.is_empty() {
                                ui.label(egui::RichText::new("No directories being watched").weak());
                            } else {
                                for dir in watched {
                                    ui.label(format!("📁 {}", dir.display()));
                                }
                            }
                        } else {
                            ui.colored_label(egui::Color32::from_rgb(255, 150, 100), "⚠ File watcher disabled");
                        }
                    }
                    
                    SettingsTab::About => {
                        ui.heading("About FlashFind");
                        ui.add_space(10.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Version:");
                            ui.label(egui::RichText::new("v1.0.0-phase2").strong());
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label("Built:");
                            ui.label(env!("CARGO_PKG_VERSION"));
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label("Architecture:");
                            ui.label(std::env::consts::ARCH);
                        });
                        
                        ui.add_space(10.0);
                        ui.label("High-performance file search for Windows");
                        ui.label(egui::RichText::new("MIT License © 2026").weak().small());
                        
                        ui.add_space(10.0);
                        if ui.link("📖 Documentation").clicked() {
                            let _ = open::that("https://github.com");
                        }
                    }
                }
            });
    }
}

impl eframe::App for FlashFindApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let total_files = self.index.read().len();
        let state = self.indexer.state();
        let is_indexing = self.indexer.is_running();
        
        // Auto-save check
        if self.config.auto_save_interval > 0 {
            let elapsed = self.last_save.elapsed();
            if elapsed >= Duration::from_secs(self.config.auto_save_interval) {
                debug!("Auto-save triggered after {}s", elapsed.as_secs());
                self.handle_save();
                self.last_save = Instant::now();
            }
        }
        
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
                        
                        if !self.results.is_empty() && ui.button("📊 Export CSV").clicked() {
                            self.export_to_csv();
                        }
                    });
                });
                
                ui.add_space(8.0);
                
                // File type filter dropdown
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    let mut filter_changed = false;
                    egui::ComboBox::from_id_source("file_type_filter")
                        .selected_text(self.file_type_filter.label())
                        .show_ui(ui, |ui| {
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::All, "All Files").clicked();
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::Documents, "Documents").clicked();
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::Images, "Images").clicked();
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::Videos, "Videos").clicked();
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::Audio, "Audio").clicked();
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::Code, "Code").clicked();
                            filter_changed |= ui.selectable_value(&mut self.file_type_filter, FileTypeFilter::Archives, "Archives").clicked();
                        });
                    
                    if filter_changed {
                        self.do_search();
                    }
                });
                
                ui.add_space(4.0);
                
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
        let mut show_settings = self.show_settings;
        if show_settings {
            egui::Window::new("⚙ Settings")
                .open(&mut show_settings)
                .resizable(false)
                .collapsible(false)
                .fixed_size([600.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    self.render_settings(ui, ctx);
                });
        }
        self.show_settings = show_settings;
        
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
fn setup_ui_style(ctx: &egui::Context, theme: Theme) {
    let visuals = match theme {
        Theme::Dark => {
            let mut v = egui::Visuals::dark();
            v.widgets.active.rounding = egui::Rounding::same(4.0);
            v.widgets.hovered.rounding = egui::Rounding::same(4.0);
            v.window_rounding = egui::Rounding::same(8.0);
            v
        }
        Theme::Light => {
            let mut v = egui::Visuals::light();
            v.widgets.active.rounding = egui::Rounding::same(4.0);
            v.widgets.hovered.rounding = egui::Rounding::same(4.0);
            v.window_rounding = egui::Rounding::same(8.0);
            v
        }
        Theme::System => {
            // Default to dark for now, could detect system preference
            let mut v = egui::Visuals::dark();
            v.widgets.active.rounding = egui::Rounding::same(4.0);
            v.widgets.hovered.rounding = egui::Rounding::same(4.0);
            v.window_rounding = egui::Rounding::same(8.0);
            v
        }
    };
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
