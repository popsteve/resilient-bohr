use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::collections::HashSet;
use rayon::prelude::*;
use eframe::egui;
use crate::models::{FileEntry, ScanProgressState, ScanResults};

struct LocalAccumulator {
    top_files: Vec<FileEntry>,
    ext_map: HashMap<String, u64>,
    dir_map: HashMap<String, u64>,
    errors: Vec<String>,
    file_count: u32,
    last_parent: Option<(std::path::PathBuf, String)>,
}

#[cfg(target_os = "windows")]
fn get_windows_file_info(path: &std::path::Path) -> Option<(u32, u64, u32)> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::fs::OpenOptionsExt;
    
    // Open file with FILE_READ_ATTRIBUTES (0x0080) and FILE_FLAG_BACKUP_SEMANTICS (0x02000000)
    let file = std::fs::OpenOptions::new()
        .access_mode(0x0080) // FILE_READ_ATTRIBUTES
        .custom_flags(0x02000000) // FILE_FLAG_BACKUP_SEMANTICS
        .open(path)
        .ok()?;
        
    let handle = file.as_raw_handle();
    if handle.is_null() {
        return None;
    }
    
    #[allow(non_snake_case)]
    #[repr(C)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }
    
    extern "system" {
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
    }
    
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    let success = unsafe { GetFileInformationByHandle(handle as *mut std::ffi::c_void, info.as_mut_ptr()) };
    if success != 0 {
        let info = unsafe { info.assume_init() };
        let file_index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
        Some((info.dwVolumeSerialNumber, file_index, info.nNumberOfLinks))
    } else {
        None
    }
}

pub fn start_scan(
    progress: Arc<ScanProgressState>,
    results: Arc<std::sync::RwLock<Option<ScanResults>>>,
    root_path: String,
    ctx: egui::Context,
) {
    // 1. Reset progress counters
    progress.reset();

    // 2. Spawn heavy scanner task in standard background thread
    let progress_scan = progress.clone();
    let results_scan = results.clone();
    let root_path_clone = root_path.clone();
    
    std::thread::spawn(move || {
        let scan_start_time = Instant::now();
        #[cfg(target_os = "windows")] let seen_ids = Arc::new(std::sync::Mutex::new(HashSet::<u128>::new()));

        let ctx_clone = ctx.clone();
        let local_res = jwalk::WalkDir::new(&root_path_clone)
            .skip_hidden(false)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| {
                match e {
                    Ok(entry) => Some(entry),
                    Err(_err) => {
                        progress_scan.errors_count.fetch_add(1, Ordering::Relaxed);
                        None
                    }
                }
            })
            .par_bridge()
            .fold(
                || LocalAccumulator {
                    top_files: Vec::new(),
                    ext_map: HashMap::new(),
                    dir_map: HashMap::new(),
                    errors: Vec::new(),
                    file_count: 0,
                    last_parent: None,
                },
                |mut acc, entry| {
                    let file_type = entry.file_type;
                    if file_type.is_symlink() {
                        return acc;
                    }

                    if file_type.is_file() {
                        let metadata = match entry.metadata() {
                            Ok(m) => m,
                            Err(err) => {
                                progress_scan.errors_count.fetch_add(1, Ordering::Relaxed);
                                acc.errors.push(format!("Error reading metadata for {:?}: {}", entry.path(), err));
                                return acc;
                            }
                        };

                        // NTFS Hard link deduplication on Windows
                        #[cfg(target_os = "windows")]
                        {
                            use std::os::windows::fs::MetadataExt;
                            // Only call expensive GetFileInformationByHandle if the file has multiple links
                            if metadata.number_of_links() > 1 {
                                if let Some((volume, file_index, _)) = get_windows_file_info(&entry.path()) {
                                    let file_id = ((volume as u128) << 64) | (file_index as u128);
                                    let mut seen = seen_ids.lock().unwrap();
                                    if !seen.insert(file_id) {
                                        // Already counted, skip to avoid double tracking
                                        return acc;
                                    }
                                }
                            }
                        }

                        let size = metadata.len();
                        let entry_path = entry.path();

                        // Lock-free updates to global progress counters
                        progress_scan.files_scanned.fetch_add(1, Ordering::Relaxed);
                        progress_scan.bytes_scanned.fetch_add(size, Ordering::Relaxed);

                        // Get extension for ext_map
                        let extension = entry_path
                            .extension()
                            .map(|e| e.to_string_lossy().to_lowercase())
                            .unwrap_or_else(|| "no extension".to_string());

                        // Accumulate extension details
                        *acc.ext_map.entry(extension.clone()).or_insert(0) += size;

                        // Accumulate size for immediate parent only (post-process will propagate up)
                        if let Some(parent) = entry_path.parent() {
                            let root_path_buf = std::path::Path::new(&root_path_clone);
                            if parent.starts_with(root_path_buf) {
                                // Simple cache for last parent path to avoid repeated string conversions
                                let parent_str = if let Some((ref last_p, ref last_p_str)) = acc.last_parent {
                                    if last_p == parent {
                                        last_p_str.clone()
                                    } else {
                                        let p_str = parent.to_string_lossy().to_string();
                                        acc.last_parent = Some((parent.to_path_buf(), p_str.clone()));
                                        p_str
                                    }
                                } else {
                                    let p_str = parent.to_string_lossy().to_string();
                                    acc.last_parent = Some((parent.to_path_buf(), p_str.clone()));
                                    p_str
                                };

                                if !parent_str.is_empty() {
                                    *acc.dir_map.entry(parent_str).or_insert(0) += size;
                                }
                            }
                        }

                        // Maintain top-100 min-heap like structure (keeping sorted list)
                        // Optimization: only perform expensive metadata calls and string allocations for top files
                        let is_top_file = if acc.top_files.len() < 100 {
                            true
                        } else {
                            size > acc.top_files[99].size
                        };

                        if is_top_file {
                            let modified = metadata.modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);

                            let name = entry.file_name().to_string_lossy().to_string();
                            let path_str = entry_path.to_string_lossy().to_string();

                            let new_entry = FileEntry {
                                name,
                                path: path_str,
                                size,
                                modified,
                                extension,
                            };

                            if acc.top_files.len() < 100 {
                                acc.top_files.push(new_entry);
                            } else {
                                acc.top_files[99] = new_entry;
                            }
                            acc.top_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
                        }

                        // Request UI repaint periodically (every 1000 files scanned by this thread)
                        acc.file_count += 1;
                        if acc.file_count % 1000 == 0 {
                            ctx_clone.request_repaint();
                        }
                    }
                    acc
                }
            )
            .reduce(
                || LocalAccumulator {
                    top_files: Vec::new(),
                    ext_map: HashMap::new(),
                    dir_map: HashMap::new(),
                    errors: Vec::new(),
                    file_count: 0,
                    last_parent: None,
                },
                |mut acc1, mut acc2| {
                    // Merge top files
                    acc1.top_files.append(&mut acc2.top_files);
                    acc1.top_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
                    if acc1.top_files.len() > 100 {
                        acc1.top_files.truncate(100);
                    }

                    // Merge extensions
                    for (ext, size) in acc2.ext_map {
                        *acc1.ext_map.entry(ext).or_insert(0) += size;
                    }

                    // Merge directory map
                    for (dir, size) in acc2.dir_map {
                        *acc1.dir_map.entry(dir).or_insert(0) += size;
                    }

                    // Merge errors
                    acc1.errors.append(&mut acc2.errors);

                    acc1
                }
            );

        // Scan duration
        let scan_duration_ms = scan_start_time.elapsed().as_millis() as u64;

        // Post-process directory sizes: propagate immediate parent sizes to all ancestors
        let mut full_dir_map: HashMap<String, u64> = HashMap::new();
        let root_path_buf = std::path::Path::new(&root_path_clone);

        for (dir_path_str, size) in local_res.dir_map {
            let dir_path = std::path::Path::new(&dir_path_str);
            let mut current = Some(dir_path);
            while let Some(path) = current {
                if !path.starts_with(root_path_buf) {
                    break;
                }
                let p_str = path.to_string_lossy().to_string();
                if p_str.is_empty() {
                    break;
                }
                *full_dir_map.entry(p_str).or_insert(0) += size;
                current = path.parent();
            }
        }

        // Compile results
        let mut ext_breakdown: Vec<(String, u64)> = local_res.ext_map.into_iter().collect();
        ext_breakdown.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        let mut dir_breakdown: Vec<(String, u64)> = full_dir_map.into_iter().collect();
        dir_breakdown.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        if dir_breakdown.len() > 20 {
            dir_breakdown.truncate(20);
        }

        let results = ScanResults {
            scanned_path: root_path_clone.clone(),
            total_size: progress_scan.bytes_scanned.load(Ordering::Relaxed),
            total_files: progress_scan.files_scanned.load(Ordering::Relaxed),
            top_files: local_res.top_files,
            ext_breakdown,
            dir_breakdown,
            scan_duration_ms,
            errors: local_res.errors,
        };

        // Write final results to shared state
        *results_scan.write().unwrap() = Some(results);

        // Turn scanning flag off and trigger final repaint
        progress_scan.scanning.store(false, Ordering::SeqCst);
        ctx.request_repaint();
    });
}
