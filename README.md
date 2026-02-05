# ⚡ FlashFind

**Lightning-fast desktop file search for Windows**

[![Release](https://img.shields.io/github/v/release/4xush/flashfind)](https://github.com/4xush/flashfind/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Windows](https://img.shields.io/badge/platform-Windows%207%2B-blue)](https://github.com/4xush/flashfind)

> 🚀 **Ultra-fast file search that's 50-200x faster than Windows Explorer**

## ✨ Features

- **⚡ Instant Search** - Sub-millisecond search across thousands of files
- **📁 Real-time Monitoring** - Automatically tracks file changes (create, modify, delete, rename)
- **🔍 Fuzzy Matching** - Find files even with typos or partial names
- **🛡️ Production-Ready** - Comprehensive edge case handling for reliability
- **💾 Memory Efficient** - On-demand index compaction, minimal footprint
- **🎯 Zero Dependencies** - Single 3.3MB executable, no installation needed
- **🔒 Safe & Reliable** - Handles temp files, partial writes, and permissions gracefully

## 🚀 Quick Start

### Download (Recommended)
1. Download [FlashFind-v1.0.0-Windows-Portable.zip](https://github.com/4xush/flashfind/releases/latest)
2. Extract anywhere
3. Run `FlashFind.exe`
4. Add folders to index and start searching!

### Build from Source
```bash
git clone https://github.com/4xush/flashfind.git
cd FlashFind-MVP
cargo build --release
.\target\release\flashfind.exe
```

## 🎯 Why FlashFind?

Windows File Explorer search is slow (1-5 seconds) because:
- Uses COM-based architecture with multiple process hops
- Relies on a monolithic SQLite index
- Has inefficient caching and ranking algorithms

FlashFind uses a direct, memory-efficient approach:
- **Direct hashmap indexing** (O(1) lookup time)
- **Parallel file system scanning** with Rayon
- **Zero COM overhead** - pure Rust implementation
- **In-memory cache** for instant results

## 📊 Performance Results
**System:** Windows 11, 31,436 indexed files

| Search Type | FlashFind | Windows Explorer | Speedup |
|-------------|-----------|------------------|---------|
| `*.pdf` | 1.22ms | ~1,000ms | **820x** |
| `*.txt` | 0.86ms | ~500ms | **581x** |
| `document` | 6.15ms | ~1,500ms | **244x** |
| `2024` | 16.27ms | ~2,000ms | **123x** |

**Average speedup: 50-200x faster**

## 🛠️ Technology Stack
- **Rust** for memory safety and performance
- **eframe/egui** for native Windows UI
- **walkdir + rayon** for parallel file system access
- **HashMap** for O(1) search operations

## �️ Edge Case Handling (v1.0.0)

FlashFind is production-ready with comprehensive reliability improvements:

### ✅ Deleted Files
- Instantly removed from search results
- No stale entries after deletion

### ✅ Temporary Files Filtered
- Office temp files (`~$*.docx`)
- Browser downloads (`.crdownload`, `.part`)
- Generic temps (`.tmp`, `.temp`)

### ✅ Partial Write Protection
- 100ms stability check before indexing
- Prevents indexing files being actively written
- Handles large file copies gracefully

### ✅ Permission Handling
- Gracefully skips locked/protected files
- No errors on system files or admin-only folders

### ✅ Memory Management
- On-demand index compaction
- Removes deleted file tombstones
- Settings → Statistics → "Compact Index"

## 💻 System Requirements

- **OS**: Windows 7 SP1 / 8 / 10 / 11 (64-bit)
- **Disk**: 50MB free space
- **RAM**: 100MB minimum
- **Dependencies**: None (single 3.3MB .exe file)

## 📂 Configuration

FlashFind stores data in `%APPDATA%\FlashFind\`:
- `config.toml` - User settings
- `index.bin` - Search index (~1MB per 10,000 files)
- `flashfind.log` - Debug logs

### Uninstall
1. Delete `FlashFind.exe`
2. Delete `C:\Users\<USERNAME>\AppData\Roaming\FlashFind`

## 🔧 Building Releases

```powershell
# Build optimized binary (3.3MB)
cargo build --release

# Create portable ZIP package
.\FlashFind-MVP\create_portable.ps1
```

The release profile uses aggressive size optimizations:
- `opt-level = "z"` - Size optimization
- `lto = true` - Link-time optimization  
- `strip = true` - Remove debug symbols
- `codegen-units = 1` - Better optimization

## 📝 License

MIT License - see [LICENSE](LICENSE) for details

## 🤝 Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## 🌟 Acknowledgments

Built with [egui](https://github.com/emilk/egui) and [Rust](https://www.rust-lang.org/)

---

**Download**: [Latest Release](https://github.com/4xush/flashfind/releases/latest) | **Source**: [GitHub](https://github.com/4xush/flashfind)
