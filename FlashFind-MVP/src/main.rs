#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! FlashFind - High-performance Windows file search utility
//! 
//! This application provides lightning-fast file search capabilities with:
//! - Memory-efficient indexing using a path pool architecture
//! - Real-time filesystem monitoring
//! - Sub-millisecond search response times
//! - Production-grade error handling and logging

mod app;
mod config;
mod error;
mod index;
mod indexer;
mod persistence;
mod watcher;

use app::FlashFindApp;
use eframe::egui;
use tracing::info;

fn main() -> eframe::Result<()> {
    info!("FlashFind v1.0.0-phase1 starting");
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_title("FlashFind - Production v1.0"),
        ..Default::default()
    };
    
    eframe::run_native(
        "FlashFind",
        options,
        Box::new(|cc| Box::new(FlashFindApp::new(cc))),
    )
}