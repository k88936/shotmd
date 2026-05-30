use std::time::{SystemTime, UNIX_EPOCH};
use image::RgbaImage;
use xcap::Monitor;
use winit::window::Window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MonitorKey {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct CapturedMonitor {
    key: MonitorKey,
    image: RgbaImage,
}

#[derive(Clone, Copy, Debug)]
struct Selection {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

pub struct ShotApp {
    monitor_snapshots: Vec<CapturedMonitor>,
    active_monitor: Option<MonitorKey>,
    texture: Option<egui::TextureHandle>,
    last_capture: Option<RgbaImage>,
    selection: Option<Selection>,
    selection_start: Option<egui::Pos2>,
    selection_end: Option<egui::Pos2>,
    last_saved: Option<String>,
    last_error: Option<String>,
}

impl ShotApp {
    pub fn new() -> Self {
        let mut app = Self {
            monitor_snapshots: Vec::new(),
            active_monitor: None,
            texture: None,
            last_capture: None,
            selection: None,
            selection_start: None,
            selection_end: None,
            last_saved: None,
            last_error: None,
        };
        app.capture_all_monitors();
        app
    }

    fn capture_all_monitors(&mut self) {
        match Monitor::all() {
            Ok(monitors) => {
                let mut snapshots = Vec::with_capacity(monitors.len());
                let mut errors = Vec::new();

                for (index, monitor) in monitors.into_iter().enumerate() {
                    let label = monitor
                        .friendly_name()
                        .or_else(|_| monitor.name())
                        .unwrap_or_else(|_| format!("Monitor {}", index + 1));

                    let key = match (
                        monitor.x(),
                        monitor.y(),
                        monitor.width(),
                        monitor.height(),
                    ) {
                        (Ok(x), Ok(y), Ok(width), Ok(height)) => MonitorKey {
                            x,
                            y,
                            width,
                            height,
                        },
                        _ => {
                            errors.push(format!("{label}: failed to read monitor geometry"));
                            continue;
                        }
                    };

                    match monitor.capture_image() {
                        Ok(image) => snapshots.push(CapturedMonitor { key, image }),
                        Err(err) => errors.push(format!("{label}: {err}")),
                    }
                }

                self.monitor_snapshots = snapshots;
                if errors.is_empty() {
                    self.last_error = None;
                } else {
                    self.last_error = Some(errors.join("; "));
                }
            }
            Err(err) => {
                self.last_error = Some(format!("Failed to list monitors: {err}"));
                self.monitor_snapshots.clear();
                self.active_monitor = None;
            }
        }
    }

    fn snapshot_for_window(&self, window: &Window) -> Option<CapturedMonitor> {
        let current = window.current_monitor().and_then(|monitor| {
            let pos = monitor.position();
            let size = monitor.size();
            let exact = self.monitor_snapshots.iter().find(|snapshot| {
                snapshot.key.x == pos.x
                    && snapshot.key.y == pos.y
                    && snapshot.key.width == size.width
                    && snapshot.key.height == size.height
            });
            exact.or_else(|| {
                self.monitor_snapshots.iter().find(|snapshot| {
                    snapshot.key.x == pos.x && snapshot.key.y == pos.y
                })
            })
        });

        if let Some(snapshot) = current {
            return Some(snapshot.clone());
        }

        let Ok(window_pos) = window.outer_position() else {
            return None;
        };

        self.monitor_snapshots.iter().find(|snapshot| {
            let left = i64::from(snapshot.key.x);
            let top = i64::from(snapshot.key.y);
            let right = left + i64::from(snapshot.key.width);
            let bottom = top + i64::from(snapshot.key.height);
            let x = i64::from(window_pos.x);
            let y = i64::from(window_pos.y);
            x >= left && x < right && y >= top && y < bottom
        }).cloned()
    }

    fn show_snapshot(&mut self, snapshot: CapturedMonitor, ctx: &egui::Context) {
        let size = [snapshot.image.width() as usize, snapshot.image.height() as usize];
        let rgba = snapshot.image.clone().into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
        match self.texture.as_mut() {
            Some(texture) => texture.set(color_image, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture(
                    "screenshot",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        self.last_capture = Some(snapshot.image.clone());
        self.selection = None;
        self.selection_start = None;
        self.selection_end = None;
        self.last_saved = None;
        self.last_error = None;
        self.active_monitor = Some(snapshot.key);
    }

    pub fn sync_capture_to_window(&mut self, window: &Window, ctx: &egui::Context) {
        let Some(snapshot) = self.snapshot_for_window(window) else {
            return;
        };

        if self.active_monitor == Some(snapshot.key) {
            return;
        }

        self.show_snapshot(snapshot, ctx);
    }

    fn selection_from_points(
        &self,
        start: egui::Pos2,
        end: egui::Pos2,
        image_rect: egui::Rect,
        image_size: egui::Vec2,
    ) -> Option<Selection> {
        if image_rect.width() <= 0.0 || image_rect.height() <= 0.0 {
            return None;
        }

        let clamp = |pos: egui::Pos2| -> egui::Pos2 {
            egui::pos2(
                pos.x.clamp(image_rect.left(), image_rect.right()),
                pos.y.clamp(image_rect.top(), image_rect.bottom()),
            )
        };

        let start = clamp(start);
        let end = clamp(end);
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);

        let image_w = image_size.x.max(1.0);
        let image_h = image_size.y.max(1.0);

        let min_u = (min_x - image_rect.left()) / image_rect.width();
        let max_u = (max_x - image_rect.left()) / image_rect.width();
        let min_v = (min_y - image_rect.top()) / image_rect.height();
        let max_v = (max_y - image_rect.top()) / image_rect.height();

        let x = (min_u * image_w).floor().clamp(0.0, image_w - 1.0) as u32;
        let y = (min_v * image_h).floor().clamp(0.0, image_h - 1.0) as u32;
        let x2 = (max_u * image_w).ceil().clamp(1.0, image_w) as u32;
        let y2 = (max_v * image_h).ceil().clamp(1.0, image_h) as u32;

        let width = x2.saturating_sub(x).max(1);
        let height = y2.saturating_sub(y).max(1);

        Some(Selection {
            x,
            y,
            width,
            height,
        })
    }

    fn selection_screen_rect(
        &self,
        selection: Selection,
        image_rect: egui::Rect,
        image_size: egui::Vec2,
    ) -> egui::Rect {
        let scale_x = image_rect.width() / image_size.x.max(1.0);
        let scale_y = image_rect.height() / image_size.y.max(1.0);
        let min = egui::pos2(
            image_rect.left() + selection.x as f32 * scale_x,
            image_rect.top() + selection.y as f32 * scale_y,
        );
        let max = egui::pos2(
            image_rect.left() + (selection.x + selection.width) as f32 * scale_x,
            image_rect.top() + (selection.y + selection.height) as f32 * scale_y,
        );
        egui::Rect::from_min_max(min, max)
    }

    fn save_selection(&mut self) {
        let selection = match self.selection {
            Some(selection) => selection,
            None => {
                self.last_error = Some("Select a region to save".to_string());
                return;
            }
        };
        let image = match &self.last_capture {
            Some(image) => image,
            None => {
                self.last_error = Some("Capture an image before saving".to_string());
                return;
            }
        };

        let cropped = image::imageops::crop_imm(image, selection.x, selection.y, selection.width, selection.height).to_image();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let filename = format!("shot-{}.png", timestamp);
        match cropped.save(&filename) {
            Ok(()) => {
                self.last_saved = Some(filename);
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(format!("Save failed: {err}"));
            }
        }
    }

    pub fn ui(&mut self, window: &Window, ui: &mut egui::Ui) {
        egui::Panel::top("controls").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let capture_enabled = !self.monitor_snapshots.is_empty();
                if ui.add_enabled(capture_enabled, egui::Button::new("Capture")).clicked() {
                    self.capture_all_monitors();
                    self.sync_capture_to_window(window, ui.ctx());
                }

                if ui.button("Refresh monitors").clicked() {
                    self.capture_all_monitors();
                    self.sync_capture_to_window(window, ui.ctx());
                }

                let save_enabled = self.selection.is_some() && self.last_capture.is_some();
                if ui
                    .add_enabled(save_enabled, egui::Button::new("Save selection"))
                    .clicked()
                {
                    self.save_selection();
                }
            });

            if let Some(saved) = &self.last_saved {
                ui.colored_label(egui::Color32::GREEN, format!("Saved {saved}"));
            }

            if let Some(error) = &self.last_error {
                ui.colored_label(egui::Color32::RED, error);
            }
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(texture) = &self.texture {
                let available = ui.available_size();
                let size = texture.size_vec2();
                let scale = (available.x / size.x).min(available.y / size.y).min(1.0);
                let image = egui::Image::new(texture)
                    .fit_to_exact_size(size * scale)
                    .sense(egui::Sense::drag());
                let response = ui.add(image);

                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.selection_start = Some(pos);
                        self.selection_end = Some(pos);
                        self.selection = None;
                    }
                }
                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.selection_end = Some(pos);
                        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                            self.selection = self.selection_from_points(start, end, response.rect, size);
                        }
                    }
                }
                if response.drag_stopped() {
                    if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                        self.selection = self.selection_from_points(start, end, response.rect, size);
                    }
                    self.selection_start = None;
                    self.selection_end = None;
                }

                if let Some(selection) = self.selection {
                    let rect = self.selection_screen_rect(selection, response.rect, size);
                    let stroke = egui::Stroke::new(2.0, egui::Color32::YELLOW);
                    ui.painter()
                        .rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
                }
            } else {
                ui.label("Click Capture to take a screenshot.");
            }
        });
    }
}