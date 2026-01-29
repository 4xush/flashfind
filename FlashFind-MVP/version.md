# Version Notes

## January 30, 2026

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
