use image::RgbaImage;
use std::sync::{Arc, RwLock};
use winit::monitor::MonitorHandle;
use winit::window::Window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorKey {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct CapturedMonitor {
    pub key: MonitorKey,
    pub image: RgbaImage,
}

#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub monitor: MonitorKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Capture,
    Record { duration_secs: u64 },
}


pub struct UiState {
    monitor_handle: Option<MonitorHandle>,
    image_size: egui::Vec2,
    image_uri: String,
    capture_bytes: Arc<RwLock<Option<Vec<u8>>>>,
    selection: Option<Selection>,
    selection_start: Option<egui::Pos2>,
    selection_end: Option<egui::Pos2>,
    show_recording_dialog: bool,
    recording_duration_secs: u64,
    recording_mode: bool,
}

impl UiState {
    pub fn new(capture_bytes: Arc<RwLock<Option<Vec<u8>>>>) -> Self {
        Self {
            monitor_handle: None,
            image_size: egui::Vec2::ZERO,
            image_uri: String::new(),
            capture_bytes,
            selection: None,
            selection_start: None,
            selection_end: None,
            show_recording_dialog: false,
            recording_duration_secs: 3,
            recording_mode: false,
        }
    }

    pub fn update_window_snapshot(
        &mut self,
        all_captures: &[CapturedMonitor],
        window: &Window,
        _ctx: &egui::Context,
    ) {

        let Some(monitor) = window.current_monitor() else {
            self.monitor_handle = None;
            return;
        };

        let key = MonitorKey {
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
        };

        match &self.monitor_handle {
            None => {
                self.monitor_handle = Some(monitor);
            }
            Some(pending) if *pending != monitor => {
                self.monitor_handle = Some(monitor);
            }
            _ => {
                return;
            }
        }

        if self.refresh_snapshot(all_captures, key) { return; }
    }

    fn refresh_snapshot(&mut self, all_captures: &[CapturedMonitor], key: MonitorKey) -> bool {
        let Some(snapshot) = all_captures.iter().find(|s| s.key == key) else {
            return true;
        };

        self.image_size = egui::vec2(snapshot.image.width() as f32, snapshot.image.height() as f32);
        let mut png_bytes = Vec::new();
        if snapshot
            .image
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .is_ok()
        {
            *self.capture_bytes.write().unwrap() = Some(png_bytes);
        }
        self.image_uri = format!("shotmd://capture?ver={}", snapshot.key.x + snapshot.key.y);
        self.reset_selection();
        false
    }

    fn reset_selection(&mut self) {
        self.selection = None;
        self.selection_start = None;
        self.selection_end = None;
    }

    pub fn try_select(
        &mut self,
        all_captures: &[CapturedMonitor],
        window: &Window,
        ui: &mut egui::Ui,
    ) -> Option<(Selection, Command)> {
        self.update_window_snapshot(all_captures, window, ui.ctx());

        // 'r' key shortcut to open recording dialog
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::R)) {
            self.show_recording_dialog = true;
        }

        let mut drag_stopped = false;
        if self.image_size.x > 0.0 && self.image_size.y > 0.0 {
            let size = self.image_size;
            let image = egui::Image::new(&self.image_uri)
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
                }
            }
            if response.drag_stopped() {
                if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                    let monitor_key = monitor_key_for_window(all_captures, window);
                    self.selection = Some(
                        selection_from_points(start, end, response.rect, size, monitor_key),
                    );
                }
                self.selection_start = None;
                self.selection_end = None;
                drag_stopped = true;
            }

            if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                let rect = egui::Rect::from_two_pos(start, end);
                let stroke = egui::Stroke::new(2.0, egui::Color32::YELLOW);
                ui.painter()
                    .rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
            }
        } else {
            ui.label("No monitors detected.");
        }

        // Recording dialog popup
        if self.show_recording_dialog {
            egui::Window::new("Recording")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Recording duration (seconds):");
                    ui.add(egui::Slider::new(&mut self.recording_duration_secs, 1..=60));
                    if ui.button("Start Recording").clicked() {
                        self.show_recording_dialog = false;
                        self.recording_mode = true;
                    }
                });
        }

        if drag_stopped {
            if let Some(selection) = self.selection {
                if self.recording_mode {
                    self.recording_mode = false;
                    return Some((selection, Command::Record { duration_secs: self.recording_duration_secs }));
                }
                return Some((selection, Command::Capture));
            }
        }


        None
    }
}


pub fn monitor_key_for_window(
    all_captures: &[CapturedMonitor],
    window: &Window,
) -> MonitorKey {
    let monitor = window.current_monitor().expect("no monitor for window");
    let key = MonitorKey {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    };
    all_captures
        .iter()
        .find(|s| s.key == key)
        .expect("monitor not found in captures")
        .key
}

fn selection_from_points(
    start: egui::Pos2,
    end: egui::Pos2,
    image_rect: egui::Rect,
    image_size: egui::Vec2,
    monitor_key: MonitorKey,
) -> Selection {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    let min_u = (min_x - image_rect.left()) / image_rect.width();
    let max_u = (max_x - image_rect.left()) / image_rect.width();
    let min_v = (min_y - image_rect.top()) / image_rect.height();
    let max_v = (max_y - image_rect.top()) / image_rect.height();

    let x = (min_u * image_size.x).floor() as u32;
    let y = (min_v * image_size.y).floor() as u32;
    let x2 = (max_u * image_size.x).ceil() as u32;
    let y2 = (max_v * image_size.y).ceil() as u32;

    let width = x2.saturating_sub(x).max(1);
    let height = y2.saturating_sub(y).max(1);

    Selection {
        x,
        y,
        width,
        height,
        monitor: monitor_key,
    }
}
