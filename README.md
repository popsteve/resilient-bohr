# Bohr Disk Space Analyzer (波尔磁盘空间分析器)

Bohr Disk Space Analyzer is a high-performance, standalone native desktop application for Windows, written entirely in Rust. It provides a visual dashboard to scan, analyze, and manage directory space usage with modern dark HSL visuals, parallel traversal, and native Recycle Bin integration.

Bohr Disk Space Analyzer 是一款采用 Rust 语言编写的高性能 Windows 原生桌面应用程序。它通过现代深色系 UI 界面、多线程并行遍历以及 Windows 原生回收站集成，为您提供可视化仪表盘，用于快速扫描、分析和管理磁盘空间占用。

---

## Key Features (核心特点)

*   **Lightning-Fast Parallel Scanning (多线程并行扫描)**:
    Leverages `jwalk` and `rayon`'s work-stealing parallel traversal thread pool to scan large directory trees at disk-limit speed.
    采用基于 `jwalk` 与 `rayon` 的工作窃取（work-stealing）并行遍历线程池，以磁盘极限速度扫描庞大的文件目录树。

*   **NTFS Hard Link Deduplication (NTFS 硬链接去重)**:
    Uses native Win32 API handles (`GetFileInformationByHandle`) to retrieve unique volume serial numbers and index IDs. It automatically dedupes hard links, ensuring file sizes are not double-counted.
    使用 Windows 原生 Win32 API 句柄接口获取唯一卷序列号与文件索引 ID，自动识别并过滤 NTFS 硬链接，防止重复统计占用空间。

*   **Modern HSL Dark Aesthetics (现代深色调视觉设计)**:
    Designed with a high-end, visual-hierarchy HSL dark theme tailored specifically for native high-DPI Windows screens.
    专为高分屏优化的现代高质感深色系（HSL）界面，具备清晰的视觉层级和微交互动画。

*   **Proportional Size Breakdown (空间占用比例条)**:
    GitHub-style proportional segmented bar highlighting usage across file categories (e.g., Archives, Videos, Images, Documents, Code) with hover details.
    类 GitHub 样式的渐变比例条与颜色图例，直观展示归档、视频、图片、文档、代码等文件类别的空间占比。

*   **Interactive Bar Charts (交互式目录柱状图)**:
    Custom-painted vector charts representing the largest subdirectories. **Double-click** any bar to instantly drill down and set it as the next scan target.
    自定义绘制的最大子目录柱状图。**双击**目录数据条即可下钻，并将其自动设为下一次扫描的路径。

*   **Safe Recycle Bin Deletion (安全的回收站删除机制)**:
    Allows deleting files securely from within the app using the native Windows Recycle Bin (`trash` API) rather than hard-deleting them. Features a strict boundary safety check against the actual scanned root and a click-absorbing modal confirmation backdrop.
    集成 Windows 原生回收站接口，允许在软件中直接将文件安全移入回收站。配备了针对实际扫描根路径的严格边界安全校验，并带有可拦截背景点击的模态确认对话框。

*   **Bilingual CJK Font Fallback (CJK 字体自动回退)**:
    Dynamically loads Windows system fonts (e.g., Microsoft YaHei, SimSun) to prevent square-box unicode encoding rendering issues for CJK folder/file names.
    动态检测并载入 Windows 系统自带字体（如微软雅黑、宋体），完美解决中日韩（CJK）文件名及路径显示为“方块字”的乱码问题。

*   **Zero-Console Popup (独立窗口运行)**:
    Configured with `#![windows_subsystem = "windows"]` to run as a clean standalone desktop GUI window without terminal popups.
    使用 Windows 子系统属性编译，双击直接运行原生桌面图形窗口，不会弹出任何 CMD 命令行黑框。

---

## Technology Stack (技术栈)

*   **Language (核心语言)**: Rust
*   **GUI Framework (图形界面)**: `egui` & `eframe` (Immediate-mode native GPU accelerated library)
*   **Concurrency (并发库)**: `rayon` (Parallel iterator library)
*   **Filesystem Walk (文件遍历)**: `jwalk` (Fast parallel directory walking)
*   **File Deletion (文件移动/删除)**: `trash` (Safe OS-native Recycle Bin bindings)
*   **File Dialogs (文件对话框)**: `rfd` (Rust File Dialogs for native folder picking)
*   **Win32 API bindings (系统接口)**: `windows-sys` (Low-level raw Windows FFI bindings)

---

## Project Structure (项目结构)

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

## Build and Run (编译与运行)

### Prerequisites (前置准备)
You will need the Rust toolchain installed. If you do not have it, install it from [rustup.rs](https://rustup.rs/).
需要安装 Rust 工具链。如果尚未安装，请从 [rustup.rs](https://rustup.rs/) 安装。

### Run in Debug mode (开发调试)
```powershell
cargo run
```

### Build Production Release (编译正式发行版)
To compile a fully optimized standalone binary with no console window:
编译完全优化的无命令行窗口的独立可执行文件：
```powershell
cargo build --release
```
The output binary will be generated at:
编译出的程序将生成在：
`target/release/disk_analyzer.exe`
