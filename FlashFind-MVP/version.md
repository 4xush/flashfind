# Version Notes

## v1.0.0-phase2 (February 2, 2026)

### CSV Export
- **Export Search Results**: One-click export to CSV with file metadata
  - Button appears when search results exist
  - Includes: Path, Filename, Extension, File Size
  - Auto-opens containing folder after export
  - Timestamped filenames: `flashfind_export_<timestamp>.csv`
  - Properly quoted CSV format for paths with commas
  
### Enhanced About Section
- **Detailed System Information**:
  - Version number with build info
  - System architecture (x86_64, etc.)
  - MIT License notice
  - Interactive documentation link
  - Professional layout with better spacing

### Configuration System
- **JSON-based Configuration**: Added persistent configuration with `Config` struct
  - Stored in `%APPDATA%\FlashFind\config.json`
  - Fields: watched_directories, auto_save_interval, max_index_size, theme, show_hidden_files, custom_exclusions
  - Automatic save on changes with graceful fallback to defaults on load errors

### Theme Support
- **Theme Selector**: Added Dark/Light/System theme toggle in Settings
  - Real-time theme switching without restart
  - Persistent theme preference across sessions
  - Visual feedback with rounded corners (4px widgets, 8px windows)

### Auto-Save Mechanism
- **Configurable Auto-Save**: Automatic periodic index persistence
  - User-configurable interval (0-60 minutes) via Settings slider
  - Default: 5 minutes (300 seconds)
  - 0 = disabled for manual control
  - Last save timestamp tracking to prevent excessive disk I/O

### Path Security
- **Path Sanitization**: Added comprehensive path validation before open operations
  - Blocks command injection characters (&, |, ;)
  - Rejects malicious UNC paths (\\\\)
  - Requires absolute paths only
  - Prevents symlink attacks and directory traversal
  - User-friendly error messages for rejected paths

### Enhanced Settings Panel
- **Interactive Configuration UI**: 
  - Theme selector with immediate visual feedback
  - Auto-save interval slider with real-time updates
  - All changes persist automatically to config.json
  - Improved About section showing v1.0.0-phase2

### Code Quality
- **Modular Architecture**: 8 modules (error, index, indexer, persistence, watcher, app, config, main)
- **0 Compiler Warnings**: Clean build with all dead code removed
- **Safe Borrow Patterns**: Fixed egui window closure borrow conflicts

### Dependencies Added
- `serde_json 1.0`: JSON serialization for configuration
- `csv 1.3`: Prepared for future export functionality

## v1.0.0-phase1 (January 31, 2026)

### Fixed Warnings
- Removed unused variable `f` and utilized `rayon::prelude::*` for the parallel search logic.

### Virtualized Rows
- Used `show_rows`. If your search finds 100,000 files, the UI will remain buttery smooth (60fps).

### Modern Layout
- Added a `TopBottomPanel` for the search bar so it stays fixed at the top while you scroll.

### Parallel Search
- The substring search now uses Rayon to scan the index keys in parallel, making searches even faster on multi-core CPUs.

### Clean Styling
- Used `RichText::weak()` for paths and `on_hover_text` for showing the full path when you hover over a file.

### Memory Management
- The `FileIndex` now uses a central pool and `u32` pointers. This is now safe for 1M+ files.

### Thread Safety
- `parking_lot` is being used for all locks.

### Clean Search
- Added an exclusion engine to keep system junk out.

### Architectural Split
- The logic is now encapsulated in `FlashFindEngine`, making it ready for automated testing.

## Phase 3 Improvements (version = "0.2.0")

### Key Highlights of Phase 3
- **Bincode Serialization:** The FileIndex is now serializable. We use bincode because it’s much faster than JSON for large indices.
- **The Watcher:** The notify crate is running in the background. If you create a file while the app is open, the index updates.
- **Persistence Logic:** On launch, the app checks %APPDATA%\FlashFind\index.bin. If it exists, it loads instantly.
- **Serialization Efficiency:** We use `#[serde(skip)]` on the seen_paths HashSet to keep the file size smaller on disk (it's fast to rebuild the HashSet from the Vec on load).

## Final Version 

### New Features
- **Context Menus:** Right-click a file to "Open Folder" or "Copy Path."
- **Keyboard Navigation:** Press Enter to open the first result, Esc to clear.
- **UI Feedback:** Visual cues for search results.

---

## Phase 1: Production Hardening (version = "1.0.0-phase1")
**Status:** ✅ COMPLETED - Ready for Commit
**Goal:** Critical reliability and safety improvements
**Build Status:** 0 errors, 0 warnings ✅

### Dependencies Added
- `thiserror` - Structured error types
- `tracing` + `tracing-subscriber` - Structured logging framework
- `tracing-appender` - Log file rotation
- `anyhow` - Error context propagation
- `known-folders` - Proper Windows system paths
- `crossbeam-channel` - Better concurrency primitives

### ✅ Completed Improvements
- [x] Comprehensive error handling with `FlashFindError` enum (no more panics)
- [x] Modular project structure (split into 7 modules: error, index, indexer, persistence, watcher, app, main)
- [x] Atomic file writes for data safety (temp file + rename)
- [x] Versioned serialization format (INDEX_VERSION constant)
- [x] Index size limits (MAX_INDEX_SIZE = 10M files)
- [x] Structured logging to file (%APPDATA%/FlashFind/flashfind.log)
- [x] Fixed concurrent indexing (batch processing, explicit lock release)
- [x] Cancellation support for background threads (Arc<AtomicBool>)
- [x] Proper Windows paths via known-folders API
- [x] Fixed compound extension search (.tar.gz support)
- [x] Result<T, E> propagation throughout codebase
- [x] Background indexer with command channel architecture

### Architecture Improvements
- **Separation of Concerns**: 
  - `error.rs` - All error types with user-friendly messages
  - `index.rs` - Core FileIndex with tests
  - `indexer.rs` - Background thread management
  - `persistence.rs` - Load/save with atomic writes
  - `watcher.rs` - Filesystem monitoring + exclusions
  - `app.rs` - UI logic and state
  - `main.rs` - Entry point (35 lines)

- **Concurrency Safety**:
  - Batch processing (1000 files per batch)
  - Explicit lock releases between batches
  - No long-running operations while holding locks
  - Channel-based command system

- **Error Recovery**:
  - Graceful handling of corrupted indices
  - Fallback to new index on load failure
  - Watcher initialization failure doesn't crash app
  - Recoverable vs non-recoverable error classification

### Performance Stats
- Build time: ~11s (debug)
- Module count: 7 (from 1 monolithic file)
- Lines of code: ~1200 (was 306)

### Logging Improvements
- **Debug builds**: All logs to file only (no console spam)
- **Release builds**: INFO level to file (cleaner performance)
- **File location**: `%APPDATA%\FlashFind\flashfind.log`
- **Rotation**: Daily log rotation for disk space management

### UI/UX Improvements
- **Settings Window**: Fully functional settings panel with:
  - Real-time index statistics (files, insertions, duplicates, searches)
  - Indexer status monitoring
  - Watched directories list
  - About section
- **Fixed Widget ID Collisions**: Unique IDs for all result rows
- **Code Cleanup**: Removed all unused code (0 warnings)

### What's Production-Ready Now:
✅ No panics - all errors handled gracefully
✅ No memory leaks - proper Arc/RwLock management
✅ No race conditions - batch processing with explicit lock releases
✅ Atomic saves - no data corruption on crashes
✅ Version checking - forward compatibility
✅ Clean codebase - 0 compiler warnings
✅ Structured logging - full audit trail in log files
✅ Proper Windows integration - known-folders API
✅ Real-time monitoring - filesystem watcher
✅ Settings panel - user visibility into system state
