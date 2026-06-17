use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: u64, // Unix timestamp in seconds
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResults {
    pub scanned_path: String,
    pub total_size: u64,
    pub total_files: u64,
    pub top_files: Vec<FileEntry>,
    pub ext_breakdown: Vec<(String, u64)>, // Sorted desc
    pub dir_breakdown: Vec<(String, u64)>, // Sorted desc
    pub scan_duration_ms: u64,
    pub errors: Vec<String>,
}

pub struct ScanProgressState {
    pub files_scanned: AtomicU64,
    pub bytes_scanned: AtomicU64,
    pub errors_count: AtomicU64,
    pub scanning: AtomicBool,
    pub scan_start: std::sync::Mutex<Option<Instant>>,
}

impl ScanProgressState {
    pub fn new() -> Self {
        Self {
            files_scanned: AtomicU64::new(0),
            bytes_scanned: AtomicU64::new(0),
            errors_count: AtomicU64::new(0),
            scanning: AtomicBool::new(false),
            scan_start: std::sync::Mutex::new(None),
        }
    }

    pub fn reset(&self) {
        use std::sync::atomic::Ordering;
        self.files_scanned.store(0, Ordering::SeqCst);
        self.bytes_scanned.store(0, Ordering::SeqCst);
        self.errors_count.store(0, Ordering::SeqCst);
        self.scanning.store(true, Ordering::SeqCst);
        *self.scan_start.lock().unwrap() = Some(Instant::now());
    }
}
