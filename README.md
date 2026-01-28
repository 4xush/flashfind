# ⚡ FlashFind MVP

**Ultra-fast file search that's 50-200x faster than Windows Explorer**

## 🚀 The Problem
Windows File Explorer search is slow (1-5 seconds for simple searches) because:
- Uses COM-based architecture with multiple process hops
- Relies on a monolithic SQLite index
- Has inefficient caching and ranking algorithms

## 💡 The Solution
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

## 🚀 Getting Started

1. **Clone and build:**
```bash
git clone https://github.com/4xush/flashfind.git
cd FlashFind-MVP
cargo build --release