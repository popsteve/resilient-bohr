#![windows_subsystem = "windows"]

mod models;
mod scanner;

use std::sync::Arc;
use std::time::Instant;
use std::sync::atomic::Ordering;
use eframe::egui;
use egui::Color32;

use models::{FileEntry, ScanResults, ScanProgressState};

struct Toast {
    message: String,
    is_error: bool,
    expires_at: Instant,
}

struct DiskAnalyzerApp {
    target_path: String,
    search_query: String,
    
    // Concurrency state
    progress: Arc<ScanProgressState>,
    results: Arc<std::sync::RwLock<Option<ScanResults>>>,
    
    // UI Local State
    sorting_column: String,
    sorting_desc: bool,
    selected_drive: String,
    available_drives: Vec<String>,
    
    // Modal state for delete
    show_delete_modal: bool,
    file_to_delete: Option<FileEntry>,
    
    // Notifications
    toasts: Vec<Toast>,
}

impl DiskAnalyzerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Set up CJK font fallback from system fonts
        let fonts = egui::FontDefinitions::default();
        #[cfg(target_os = "windows")]
        {
            let font_paths = [
                "C:\\Windows\\Fonts\\msyh.ttc",     // Microsoft YaHei
                "C:\\Windows\\Fonts\\msyh.ttf",
                "C:\\Windows\\Fonts\\simsun.ttc",   // SimSun
                "C:\\Windows\\Fonts\\simhei.ttf",   // SimHei
            ];
            for path in &font_paths {
                if let Ok(font_bytes) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "cjk_fallback".to_owned(),
                        egui::FontData::from_owned(font_bytes),
                    );
                    
                    if let Some(vec) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                        vec.insert(0, "cjk_fallback".to_owned());
                    }
                    if let Some(vec) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                        vec.insert(0, "cjk_fallback".to_owned());
                    }
                    break;
                }
            }
        }
        cc.egui_ctx.set_fonts(fonts);

        // Detect system drives
        let mut drives = Vec::new();
        #[cfg(target_os = "windows")]
        {
            for drive_char in b'A'..=b'Z' {
                let drive_path = format!("{}:\\", drive_char as char);
                if std::path::Path::new(&drive_path).exists() {
                    drives.push(drive_path);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            drives.push("/".to_string());
        }

        // Default to current directory or first drive
        let default_path = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| drives.first().cloned().unwrap_or_else(|| "C:\\".to_string()));

        let selected_drive = drives.first().cloned().unwrap_or_default();

        Self {
            target_path: default_path,
            search_query: String::new(),
            progress: Arc::new(ScanProgressState::new()),
            results: Arc::new(std::sync::RwLock::new(None)),
            sorting_column: "size".to_string(),
            sorting_desc: true,
            selected_drive,
            available_drives: drives,
            show_delete_modal: false,
            file_to_delete: None,
            toasts: Vec::new(),
        }
    }

    fn add_toast(&mut self, message: &str, is_error: bool) {
        self.toasts.clear();
        self.toasts.push(Toast {
            message: message.to_string(),
            is_error,
            expires_at: Instant::now() + std::time::Duration::from_secs(4),
        });
    }
}

// Helpers for formatted values
fn format_bytes(bytes: u64) -> String {
    if bytes == 0 { return "0 Bytes".to_string(); }
    const K: f64 = 1024.0;
    let sizes = ["Bytes", "KB", "MB", "GB", "TB", "PB"];
    let i = (bytes as f64).log(K).floor() as usize;
    let i = i.min(sizes.len() - 1);
    let val = (bytes as f64) / K.powi(i as i32);
    format!("{:.2} {}", val, sizes[i])
}

fn format_number(num: u64) -> String {
    let s = num.to_string();
    let mut result = String::new();
    let count = s.len();
    for (i, c) in s.chars().enumerate() {
        result.push(c);
        let pos = count - 1 - i;
        if pos > 0 && pos % 3 == 0 {
            result.push(',');
        }
    }
    result
}

fn format_date(timestamp: u64) -> String {
    if timestamp == 0 { return "Unknown".to_string(); }
    let secs = timestamp;
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    
    // Basic Gregorian calendar calculator
    let mut year = 1970;
    let mut day_count = days;
    loop {
        let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let year_days = if is_leap { 366 } else { 365 };
        if day_count >= year_days {
            day_count -= year_days;
            year += 1;
        } else {
            break;
        }
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_days = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &m_days in month_days.iter() {
        if day_count >= m_days {
            day_count -= m_days;
            month += 1;
        } else {
            break;
        }
    }
    let day = day_count + 1;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

fn get_ext_color(ext: &str) -> Color32 {
    match ext {
        "zip" | "rar" | "7z" | "tar" | "gz" | "iso" => Color32::from_rgb(168, 85, 247), // Purple
        "mp4" | "mkv" | "avi" | "mov" | "flv" | "wmv" => Color32::from_rgb(59, 130, 246), // Blue
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => Color32::from_rgb(236, 72, 153), // Pink
        "exe" | "msi" | "bat" | "cmd" | "ps1" | "sh" | "dll" | "sys" => Color32::from_rgb(239, 68, 68), // Red
        "txt" | "md" | "json" | "xml" | "yaml" | "yml" | "ini" | "log" => Color32::from_rgb(245, 158, 11), // Yellow
        "rs" | "go" | "py" | "js" | "ts" | "html" | "css" | "cpp" | "c" | "h" => Color32::from_rgb(6, 182, 212), // Cyan
        "no extension" => Color32::from_rgb(100, 116, 139), // Slate-grey
        _ => Color32::from_rgb(20, 184, 166), // Teal (Other)
    }
}

fn reveal_in_explorer(path: &str) {
    let p = std::path::Path::new(path);
    if p.exists() {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("explorer")
                .raw_arg(format!(r#"/select,"{}""#, p.to_string_lossy()))
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = open::that(p);
        }
    }
}

// Styling Frame for glassmorphic-like cards
fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(Color32::from_rgb(30, 35, 48))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(45, 50, 68)))
        .rounding(8.0)
        .inner_margin(16.0)
}

// Color Dot helper for legends
fn draw_color_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
}

impl eframe::App for DiskAnalyzerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply custom visual adjustments once
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(20, 25, 35);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 35, 48);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 45, 62);
        visuals.widgets.active.bg_fill = Color32::from_rgb(50, 55, 75);
        visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(226, 232, 240); // Text
        ctx.set_visuals(visuals);

        // Remove expired toasts
        self.toasts.retain(|t| t.expires_at > Instant::now());

        let is_scanning = self.progress.scanning.load(Ordering::Relaxed);
        if !is_scanning {
            self.toasts.retain(|t| t.message != "Scanning initiated...");
        }

        // Central Panel
        egui::CentralPanel::default().show(ctx, |ui| {
            // Top thin progress bar to prevent layout shifting
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 4.0),
                egui::Sense::hover(),
            );
            if is_scanning {
                let elapsed = self.progress.scan_start.lock().unwrap()
                    .map(|s| s.elapsed().as_secs_f64())
                    .unwrap_or(0.0);
                
                // Draw a background track
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(30, 35, 48));
                
                // Draw an animating indicator segment
                let width = rect.width();
                let bar_width = width * 0.3; // 30% of total width
                let x_pos = rect.min.x + ((elapsed * 300.0) % (width as f64 + bar_width as f64)) as f32 - bar_width;
                let indicator_rect = egui::Rect::from_min_max(
                    egui::pos2(x_pos.max(rect.min.x), rect.min.y),
                    egui::pos2((x_pos + bar_width).min(rect.max.x), rect.max.y),
                );
                
                if indicator_rect.width() > 0.0 {
                    ui.painter().rect_filled(indicator_rect, 2.0, Color32::from_rgb(20, 184, 166)); // Teal loading segment
                }
            } else {
                // Just draw a thin divider line to keep height exactly the same
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(30, 35, 48));
            }
            ui.add_space(8.0);
            
            // Header / App Title
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("💾").size(32.0));
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Bohr Disk Analyzer").size(22.0).strong());
                    ui.label(egui::RichText::new("High-performance native file size scanner").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                });
            });
            ui.add_space(12.0);

            // Control Panel Card
            card_frame().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Drive Select Dropdown
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Drive").size(11.0).strong().color(Color32::from_rgb(148, 163, 184)));
                        ui.add_space(2.0);
                        let prev_drive = self.selected_drive.clone();
                        egui::ComboBox::from_id_salt("drive_combobox")
                            .selected_text(&self.selected_drive)
                            .width(65.0)
                            .show_ui(ui, |ui| {
                                for drive in &self.available_drives {
                                    ui.selectable_value(&mut self.selected_drive, drive.clone(), drive);
                                }
                            });
                        if prev_drive != self.selected_drive {
                            self.target_path = self.selected_drive.clone();
                        }
                    });

                    // Path Input Field & Browse Button
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Scan Directory").size(11.0).strong().color(Color32::from_rgb(148, 163, 184)));
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            // Leave 130.0 (Actions) + 8.0 (spacing) + 85.0 (Browse) + 8.0 (spacing) + 16.0 (right padding) = 247.0
                            let text_edit_w = (ui.available_width() - 247.0).max(100.0);
                            ui.add_sized(
                                egui::vec2(text_edit_w, 24.0),
                                egui::TextEdit::singleline(&mut self.target_path)
                                    .hint_text("e.g. C:\\Users\\Name"),
                            );

                            ui.add_enabled_ui(!is_scanning, |ui| {
                                if ui.add_sized(egui::vec2(85.0, 24.0), egui::Button::new("Browse 📂")).clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_title("Select Directory to Scan")
                                        .pick_folder()
                                    {
                                        self.target_path = path.to_string_lossy().to_string();
                                    }
                                }
                            });
                        });
                    });

                    // Scan Trigger Button
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Actions").size(11.0).strong().color(Color32::from_rgb(148, 163, 184)));
                        ui.add_space(2.0);
                        
                        let scan_btn_text = if is_scanning { "Scanning..." } else { "Start Scan ⚡" };
                        let btn = ui.add_sized(
                            egui::vec2(130.0, 24.0),
                            egui::Button::new(scan_btn_text)
                                .fill(if is_scanning { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(20, 184, 166) }),
                        );

                        if btn.clicked() && !is_scanning {
                            if self.target_path.trim().is_empty() {
                                self.add_toast("Please enter a directory path to scan", true);
                            } else if !std::path::Path::new(&self.target_path).exists() {
                                self.add_toast("Path does not exist", true);
                            } else {
                                self.add_toast("Scanning initiated...", false);
                                crate::scanner::start_scan(
                                    self.progress.clone(),
                                    self.results.clone(),
                                    self.target_path.clone(),
                                    ctx.clone(),
                                );
                            }
                        }
                    });
                });
            });
            ui.add_space(16.0);

            // Progress Banner (completely removed to prevent layout shifting)

            // Stats Cards Grid (live progress during scan, or final results)
            let results_opt = self.results.read().unwrap().clone();
            let has_results = results_opt.is_some();
            
            ui.columns(4, |columns| {
                let (size_str, files_str, avg_str, dur_str) = if is_scanning {
                    let files = self.progress.files_scanned.load(Ordering::Relaxed);
                    let bytes = self.progress.bytes_scanned.load(Ordering::Relaxed);
                    let avg = if files > 0 { bytes / files } else { 0 };
                    let elapsed_s = self.progress.scan_start.lock().unwrap()
                        .map(|s| s.elapsed().as_secs_f64())
                        .unwrap_or(0.0);
                    let speed = if elapsed_s > 0.1 {
                        (files as f64 / elapsed_s) as u64
                    } else {
                        0
                    };
                    let dur_str = if speed > 0 {
                        format!("{:.2}s ({} f/s)", elapsed_s, format_number(speed))
                    } else {
                        format!("{:.2}s", elapsed_s)
                    };
                    (
                        format_bytes(bytes),
                        format_number(files),
                        format_bytes(avg),
                        dur_str,
                    )
                } else if let Some(ref res) = results_opt {
                    let avg = if res.total_files > 0 { res.total_size / res.total_files } else { 0 };
                    (
                        format_bytes(res.total_size),
                        format_number(res.total_files),
                        format_bytes(avg),
                        format!("{:.2}s", res.scan_duration_ms as f64 / 1000.0),
                    )
                } else {
                    ("0 Bytes".to_string(), "0".to_string(), "0 Bytes".to_string(), "0.00s".to_string())
                };

                card_frame().show(&mut columns[0], |ui| {
                    ui.label(egui::RichText::new("📁 Total Size").small().weak());
                    ui.label(egui::RichText::new(size_str).monospace().strong().size(18.0));
                });
                card_frame().show(&mut columns[1], |ui| {
                    ui.label(egui::RichText::new("📄 Files Scanned").small().weak());
                    ui.label(egui::RichText::new(files_str).monospace().strong().size(18.0));
                });
                card_frame().show(&mut columns[2], |ui| {
                    ui.label(egui::RichText::new("📊 Average File Size").small().weak());
                    ui.label(egui::RichText::new(avg_str).monospace().strong().size(18.0));
                });
                card_frame().show(&mut columns[3], |ui| {
                    ui.label(egui::RichText::new("⏱️ Duration").small().weak());
                    ui.label(egui::RichText::new(dur_str).monospace().strong().size(18.0));
                });
            });
            ui.add_space(16.0);

            // Main Results Dashboard (Charts + Tables)
            if has_results {
                if let Some(ref res) = results_opt {
                    // Charts block
                    ui.columns(2, |columns| {
                        // Left Column: Extensions Breakdown
                        card_frame().show(&mut columns[0], |ui| {
                            ui.set_min_height(220.0);
                            ui.label(egui::RichText::new("File Types Breakdown").strong());
                            ui.add_space(8.0);
                            
                            // Segmented Bar
                            let total_size = res.total_size;
                            if total_size > 0 {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 16.0),
                                    egui::Sense::hover(),
                                );
                                let painter = ui.painter();
                                let mut current_x = rect.min.x;
                                let total_width = rect.width();
                                
                                let slice_count = res.ext_breakdown.len().min(7);
                                let mut segments = Vec::new();
                                let mut accumulated_size = 0;
                                for i in 0..slice_count {
                                    let (ext, size) = &res.ext_breakdown[i];
                                    segments.push((ext.clone(), *size));
                                    accumulated_size += size;
                                }
                                if res.ext_breakdown.len() > 7 {
                                    let other_size = total_size - accumulated_size;
                                    if other_size > 0 {
                                        segments.push(("Other".to_string(), other_size));
                                    }
                                }

                                for (idx, (ext, size)) in segments.iter().enumerate() {
                                    let fraction = (*size as f64) / (total_size as f64);
                                    let seg_width = total_width * fraction as f32;
                                    if seg_width < 1.0 { continue; }

                                    let seg_rect = egui::Rect::from_min_max(
                                        egui::pos2(current_x, rect.min.y),
                                        egui::pos2((current_x + seg_width).min(rect.max.x), rect.max.y),
                                    );

                                    let is_first = idx == 0;
                                    let is_last = idx == segments.len() - 1;
                                    let rounding = egui::Rounding {
                                        nw: if is_first { 4.0 } else { 0.0 },
                                        sw: if is_first { 4.0 } else { 0.0 },
                                        ne: if is_last { 4.0 } else { 0.0 },
                                        se: if is_last { 4.0 } else { 0.0 },
                                    };

                                    painter.rect_filled(seg_rect, rounding, get_ext_color(ext));
                                    current_x += seg_width;
                                }
                            }
                            ui.add_space(12.0);

                            // Legend Grid
                            egui::ScrollArea::vertical()
                                .id_salt("legend_scroll")
                                .max_height(160.0)
                                .show(ui, |ui| {
                                    let total_size = res.total_size.max(1);
                                    for (ext, size) in res.ext_breakdown.iter().take(8) {
                                        ui.horizontal(|ui| {
                                            draw_color_dot(ui, get_ext_color(ext));
                                            ui.label(egui::RichText::new(ext.to_uppercase()).strong().small());
                                            
                                            let pct = (*size as f64 / total_size as f64) * 100.0;
                                            ui.label(egui::RichText::new(format!("{:.1}%", pct)).small().weak());
                                            
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(format_bytes(*size)).small().monospace());
                                            });
                                        });
                                        ui.add_space(2.0);
                                    }
                                });
                        });

                        // Right Column: Directories List Custom Bar Chart
                        card_frame().show(&mut columns[1], |ui| {
                            ui.set_min_height(220.0);
                            ui.label(egui::RichText::new("Largest Directories").strong());
                            ui.add_space(8.0);
                            
                            if res.dir_breakdown.is_empty() {
                                ui.centered_and_justified(|ui| {
                                    ui.label(egui::RichText::new("No directory details").weak().italics());
                                });
                            } else {
                                let max_size = res.dir_breakdown.iter().map(|x| x.1).max().unwrap_or(1);
                                egui::ScrollArea::vertical()
                                    .id_salt("dir_chart_scroll")
                                    .max_height(160.0)
                                    .show(ui, |ui| {
                                        for (dir_path, size) in &res.dir_breakdown {
                                            let dir_row_response = ui.horizontal(|ui| {
                                                // Path Label
                                                let display_path = if dir_path.len() > 30 {
                                                    format!("...{}", &dir_path[dir_path.len() - 30..])
                                                } else {
                                                    dir_path.clone()
                                                };

                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(130.0, 20.0),
                                                    egui::Layout::left_to_right(egui::Align::Center),
                                                    |ui| {
                                                        let label = ui.label(egui::RichText::new(display_path).small().weak());
                                                        if label.hovered() {
                                                            label.on_hover_text(dir_path);
                                                        }
                                                    }
                                                );

                                                // Custom width based on size ratio
                                                let fraction = (*size as f64) / (max_size as f64);
                                                let avail_width = ui.available_width() - 80.0;
                                                let bar_width = (avail_width * fraction as f32).max(4.0);

                                                let (rect, mut response) = ui.allocate_exact_size(
                                                    egui::vec2(bar_width, 14.0),
                                                    egui::Sense::click(),
                                                );

                                                let hover = response.hovered();
                                                let color = if hover {
                                                    Color32::from_rgb(20, 184, 166) // bright teal
                                                } else {
                                                    Color32::from_rgba_unmultiplied(20, 184, 166, 120) // muted teal
                                                };

                                                ui.painter().rect_filled(rect, 3.0, color);

                                                if hover {
                                                    response = response.on_hover_text(format!("Double-click to set as target\nSize: {}", format_bytes(*size)));
                                                }

                                                if response.double_clicked() {
                                                    self.target_path = dir_path.clone();
                                                    self.add_toast("Target path updated", false);
                                                }

                                                // Size Text
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    ui.label(egui::RichText::new(format_bytes(*size)).small().strong());
                                                });
                                            }).response;
                                             let full_dir_response = ui.interact(
                                                 dir_row_response.rect,
                                                 dir_row_response.id.with("full_dir"),
                                                 egui::Sense::click(),
                                             );
                                             let combined_dir_response = dir_row_response.union(full_dir_response);
                                             combined_dir_response.context_menu(|ui| {
                                                 if ui.button("📂 Reveal in Explorer").clicked() {
                                                     reveal_in_explorer(dir_path);
                                                     ui.close_menu();
                                                 }
                                                 if ui.button("⚡ Target for Scan").clicked() {
                                                     self.target_path = dir_path.clone();
                                                     self.add_toast("Target path updated", false);
                                                     ui.close_menu();
                                                 }
                                             });
                                             ui.add_space(4.0);
                                        }
                                    });
                            }
                        });
                    });
                    ui.add_space(16.0);

                    // Table Section: Top 100 Files
                    card_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Top 100 Largest Files").strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Filter files..."));
                            });
                        });
                        ui.add_space(8.0);

                        // File filter & sort
                        let mut files = res.top_files.clone();
                        if !self.search_query.is_empty() {
                            let query = self.search_query.to_lowercase();
                            files.retain(|f| f.name.to_lowercase().contains(&query) || f.path.to_lowercase().contains(&query));
                        }

                        let col = self.sorting_column.clone();
                        let desc = self.sorting_desc;
                        files.sort_by(|a, b| {
                            let cmp = match col.as_str() {
                                "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                "path" => a.path.to_lowercase().cmp(&b.path.to_lowercase()),
                                "modified" => a.modified.cmp(&b.modified),
                                "extension" => a.extension.cmp(&b.extension),
                                _ => a.size.cmp(&b.size),
                            };
                            if desc { cmp.reverse() } else { cmp }
                        });

                        // Table Layout (Dynamic widths based on window size)
                        let total_width = ui.available_width() - 16.0;
                        let w_name = total_width * 0.22;
                        let w_path = total_width * 0.40;
                        let w_size = total_width * 0.12;
                        let w_date = total_width * 0.15;
                        let w_ext = total_width * 0.07;
                        let w_act = total_width * 0.04;

                        // Header Row
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(w_name, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                if ui.button(format!("Name {}", if col == "name" { if desc { "↓" } else { "↑" } } else { "↕" })).clicked() {
                                    self.sorting_column = "name".to_string();
                                    self.sorting_desc = !self.sorting_desc;
                                }
                            });
                            ui.allocate_ui_with_layout(egui::vec2(w_path, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                if ui.button(format!("Full Path {}", if col == "path" { if desc { "↓" } else { "↑" } } else { "↕" })).clicked() {
                                    self.sorting_column = "path".to_string();
                                    self.sorting_desc = !self.sorting_desc;
                                }
                            });
                            ui.allocate_ui_with_layout(egui::vec2(w_size, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                if ui.button(format!("Size {}", if col == "size" { if desc { "↓" } else { "↑" } } else { "↕" })).clicked() {
                                    self.sorting_column = "size".to_string();
                                    self.sorting_desc = !self.sorting_desc;
                                }
                            });
                            ui.allocate_ui_with_layout(egui::vec2(w_date, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                if ui.button(format!("Modified {}", if col == "modified" { if desc { "↓" } else { "↑" } } else { "↕" })).clicked() {
                                    self.sorting_column = "modified".to_string();
                                    self.sorting_desc = !self.sorting_desc;
                                }
                            });
                            ui.allocate_ui_with_layout(egui::vec2(w_ext, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                if ui.button(format!("Ext {}", if col == "extension" { if desc { "↓" } else { "↑" } } else { "↕" })).clicked() {
                                    self.sorting_column = "extension".to_string();
                                    self.sorting_desc = !self.sorting_desc;
                                }
                            });
                            ui.allocate_ui_with_layout(egui::vec2(w_act, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.label("Act");
                            });
                        });
                        ui.add_space(4.0);
                        ui.separator();

                        // Scrollable Rows
                        egui::ScrollArea::vertical()
                            .id_salt("files_table_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if files.is_empty() {
                                    ui.centered_and_justified(|ui| {
                                        ui.label(egui::RichText::new("No files match the filter").weak().italics());
                                    });
                                } else {
                                    for file in files {
                                        let file_row_response = ui.horizontal(|ui| {
                                            // Name
                                            ui.allocate_ui_with_layout(egui::vec2(w_name, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                let file_icon = match file.extension.as_str() {
                                                    "zip" | "rar" | "7z" | "tar" | "gz" | "iso" => "📦",
                                                    "mp4" | "mkv" | "avi" | "mov" | "flv" | "wmv" => "🎥",
                                                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "🖼️",
                                                    "exe" | "msi" | "bat" | "cmd" | "ps1" | "sh" | "dll" | "sys" => "⚙️",
                                                    _ => "📄",
                                                };
                                                ui.label(format!("{} {}", file_icon, file.name));
                                            });

                                            // Path
                                            ui.allocate_ui_with_layout(egui::vec2(w_path, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                let lbl = ui.label(egui::RichText::new(&file.path).monospace().small().color(Color32::from_rgb(148, 163, 184)));
                                                if lbl.hovered() {
                                                    lbl.on_hover_text(&file.path);
                                                }
                                            });

                                            // Size
                                            ui.allocate_ui_with_layout(egui::vec2(w_size, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(format_bytes(file.size)).strong().monospace());
                                            });

                                            // Date
                                            ui.allocate_ui_with_layout(egui::vec2(w_date, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(format_date(file.modified)).small().color(Color32::from_rgb(148, 163, 184)));
                                            });

                                            // Ext
                                            ui.allocate_ui_with_layout(egui::vec2(w_ext, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(&file.extension).code());
                                            });

                                            // Trash Button
                                            ui.allocate_ui_with_layout(egui::vec2(w_act, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                let delete_btn = ui.add(egui::Button::new("🗑").fill(Color32::TRANSPARENT));
                                                if delete_btn.on_hover_text("Move to Recycle Bin").clicked() {
                                                    self.file_to_delete = Some(file.clone());
                                                    self.show_delete_modal = true;
                                                }
                                            });
                                        }).response;
                                         let full_row_rect = egui::Rect::from_min_max(
                                             file_row_response.rect.min,
                                             egui::pos2(file_row_response.rect.max.x - w_act - 4.0, file_row_response.rect.max.y),
                                         );
                                         let full_row_response = ui.interact(
                                             full_row_rect,
                                             file_row_response.id.with("full_row"),
                                             egui::Sense::click(),
                                         );
                                         full_row_response.context_menu(|ui| {
                                             if ui.button("📂 Reveal in Explorer").clicked() {
                                                 reveal_in_explorer(&file.path);
                                                 ui.close_menu();
                                             }
                                             if ui.button("🗑 Move to Recycle Bin").clicked() {
                                                 self.file_to_delete = Some(file.clone());
                                                 self.show_delete_modal = true;
                                                 ui.close_menu();
                                             }
                                         });
                                         ui.separator();
                                    }
                                }
                            });
                    });
                }
            } else if !is_scanning {
                // Initial State Instructions
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Enter a directory path above and click 'Start Scan' to begin.").weak().italics());
                });
            }
            if self.show_delete_modal {
                let screen_rect = ui.ctx().screen_rect();
                let _response = ui.interact(
                    screen_rect,
                    ui.id().with("modal_backdrop"),
                    egui::Sense::click(),
                );
                ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(160));
            }
        });

        // Custom Delete Confirmation Modal Overlay
        if self.show_delete_modal {
            if let Some(file) = self.file_to_delete.clone() {

                let modal_title = "Move File to Recycle Bin?";
                egui::Window::new(modal_title)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .resizable(false)
                    .collapsible(false)
                    .title_bar(true)
                    .show(ctx, |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(450.0, 180.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.label(egui::RichText::new("Are you sure you want to move this file to the Windows Recycle Bin? You can restore it later if needed.").color(Color32::from_rgb(226, 232, 240)));
                                ui.add_space(12.0);

                                // Details box
                                card_frame().show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Name:").strong().weak());
                                        ui.label(egui::RichText::new(&file.name).strong());
                                    });
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Size:").strong().weak());
                                        ui.label(egui::RichText::new(format_bytes(file.size)).strong().color(Color32::from_rgb(20, 184, 166)));
                                    });
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Path:").strong().weak());
                                        ui.label(egui::RichText::new(&file.path).monospace().small());
                                    });
                                });

                                ui.add_space(16.0);
                                ui.horizontal(|ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Confirm Button
                                        let delete_btn = egui::Button::new(egui::RichText::new("Move to Recycle Bin").color(Color32::WHITE))
                                            .fill(Color32::from_rgb(239, 68, 68));

                                        if ui.add(delete_btn).clicked() {
                                            let path = std::path::Path::new(&file.path);
                                            
                                            // Boundary Check
                                            let is_inside_root = if let Some(ref scanned_res) = *self.results.read().unwrap() {
                                                 // Check if file is descendant of actual scanned path
                                                 let root_p = std::path::Path::new(&scanned_res.scanned_path);
                                                 path.starts_with(root_p)
                                             } else {
                                                 false
                                             };

                                            if !is_inside_root {
                                                self.add_toast("Security Check Failed: File is outside scanned directory.", true);
                                                self.show_delete_modal = false;
                                                self.file_to_delete = None;
                                                return;
                                            }

                                            // Check physical file exists
                                            if !path.exists() {
                                                self.add_toast("File no longer exists on disk.", true);
                                                self.show_delete_modal = false;
                                                self.file_to_delete = None;
                                                return;
                                            }

                                            // Execute trash move
                                            match trash::delete(path) {
                                                Ok(_) => {
                                                    self.add_toast("File moved to Recycle Bin", false);
                                                    
                                                    // Local sync stats
                                                    let mut res_guard = self.results.write().unwrap();
                                                    if let Some(ref mut res) = *res_guard {
                                                        res.top_files.retain(|f| f.path != file.path);
                                                        res.total_size = res.total_size.saturating_sub(file.size);
                                                        res.total_files = res.total_files.saturating_sub(1);
                                                    }
                                                }
                                                Err(e) => {
                                                    let error_str = e.to_string();
                                                    let friendly_err = if error_str.contains("CanonicalizePath") {
                                                        "Failed to delete file. The file may be locked by another application or protected by permissions.".to_string()
                                                    } else {
                                                        format!("Failed to delete file: {}", e)
                                                    };
                                                    self.add_toast(&friendly_err, true);
                                                }
                                            }

                                            self.show_delete_modal = false;
                                            self.file_to_delete = None;
                                        }

                                        ui.add_space(8.0);

                                        // Cancel Button
                                        if ui.button("Cancel").clicked() {
                                            self.show_delete_modal = false;
                                            self.file_to_delete = None;
                                        }
                                    });
                                });
                            }
                        );
                    });
            }
        }

        // Draw Notification Toast Banners
        if !self.toasts.is_empty() {
            let screen_rect = ctx.screen_rect();
            let mut toast_pos = egui::pos2(screen_rect.max.x - 24.0, screen_rect.max.y - 24.0);
            
            for toast in &self.toasts {
                let text = egui::RichText::new(&toast.message)
                    .color(Color32::WHITE)
                    .size(13.0);
                
                let bg_color = if toast.is_error {
                    Color32::from_rgb(239, 68, 68) // Red
                } else {
                    Color32::from_rgb(20, 184, 166) // Teal
                };

                // Allocate a window area for the toast (anchored at the bottom-right)
                let size = egui::vec2(280.0, 48.0);
                toast_pos.y -= size.y + 12.0;

                egui::Area::new(egui::Id::new(&toast.message))
                    .fixed_pos(toast_pos - egui::vec2(size.x, 0.0))
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(bg_color)
                            .rounding(6.0)
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.add_sized(size - egui::vec2(24.0, 24.0), egui::Label::new(text));
                            });
                    });
            }
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bohr Disk Analyzer")
            .with_inner_size([1020.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Bohr Disk Analyzer",
        options,
        Box::new(|cc| {
            Ok(Box::new(DiskAnalyzerApp::new(cc)))
        }),
    )
}
