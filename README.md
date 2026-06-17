# Bohr Disk Space Analyzer

Bohr Disk Space Analyzer is a high-performance, standalone native desktop application for Windows, written entirely in Rust. It provides a visual dashboard to scan, analyze, and manage directory space usage with modern dark HSL visuals, parallel traversal, and native Recycle Bin integration.

---

## Key Features

*   **Lightning-Fast Parallel Scanning**:
    Leverages `jwalk` and `rayon`'s work-stealing parallel traversal thread pool to scan large directory trees at disk-limit speed.

*   **NTFS Hard Link Deduplication**:
    Uses native Win32 API handles (`GetFileInformationByHandle`) to retrieve unique volume serial numbers and index IDs. It automatically dedupes hard links, ensuring file sizes are not double-counted.

*   **Modern HSL Dark Aesthetics**:
    Designed with a high-end, visual-hierarchy HSL dark theme tailored specifically for native high-DPI Windows screens.

*   **Proportional Size Breakdown**:
    GitHub-style proportional segmented bar highlighting usage across file categories (e.g., Archives, Videos, Images, Documents, Code) with hover details.

*   **Interactive Bar Charts**:
    Custom-painted vector charts representing the largest subdirectories. **Double-click** any bar to instantly drill down and set it as the next scan target.

*   **Safe Recycle Bin Deletion**:
    Allows deleting files securely from within the app using the native Windows Recycle Bin (`trash` API) rather than hard-deleting them. Features a strict boundary safety check against the actual scanned root and a click-absorbing modal confirmation backdrop.

*   **Bilingual CJK Font Fallback**:
    Dynamically loads Windows system fonts (e.g., Microsoft YaHei, SimSun) to prevent square-box unicode encoding rendering issues for CJK folder/file names.

*   **Zero-Console Popup**:
    Configured with `#![windows_subsystem = "windows"]` to run as a clean standalone desktop GUI window without terminal popups.

---

## Technology Stack

*   **Language**: Rust
*   **GUI Framework**: `egui` & `eframe` (Immediate-mode native GPU accelerated library)
*   **Concurrency**: `rayon` (Parallel iterator library)
*   **Filesystem Walk**: `jwalk` (Fast parallel directory walking)
*   **File Deletion**: `trash` (Safe OS-native Recycle Bin bindings)
*   **File Dialogs**: `rfd` (Rust File Dialogs for native folder picking)
*   **Win32 API bindings**: `windows-sys` (Low-level raw Windows FFI bindings)

---

## Project Structure

```
resilient-bohr/
├── Cargo.toml          # Dependency declarations (egui, trash, jwalk, rayon)
├── src/
│   ├── main.rs         # Entry point, visual theme, layout components, and modal handlers
│   ├── models.rs       # Struct definitions (FileEntry, ScanResults, ScanProgressState)
│   └── scanner.rs      # Rayon parallel walk engine and NTFS hard-link deduction
├── disk_analyzer.exe   # Compiled standalone executable in root directory
└── README.md           # This documentation
```

---

## Build and Run

### Prerequisites
You will need the Rust toolchain installed. If you do not have it, install it from [rustup.rs](https://rustup.rs/).

### Run in Debug mode
```powershell
cargo run
```

### Build Production Release
To compile a fully optimized standalone binary with no console window:
```powershell
cargo build --release
```
The output binary will be generated at:
`target/release/disk_analyzer.exe`
