mod ui;

use anyhow::Context;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;
use egui::load::{BytesLoadResult, BytesLoader, BytesPoll, LoadError};

struct App {
    ui_state: ui::UiState,
    all_captures: Vec<ui::CapturedMonitor>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            return;
        };

        let mut result = None;
        egui::CentralPanel::no_frame().show_inside(ui, |ui| {
            result = self
                .ui_state
                .try_select(&self.all_captures, window, ui);
        });

        if let Some((selection, command)) = result {
            match command {
                ui::Command::Capture => {
                    save_selection(&selection, &self.all_captures).expect("save_selection failed");
                }
                ui::Command::Record => todo!(),
            }
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn save_selection(
    selection: &ui::Selection,
    all_captures: &[ui::CapturedMonitor],
) -> anyhow::Result<String> {
    let captured = all_captures
        .iter()
        .find(|c| c.key == selection.monitor)
        .context("Selection monitor not found in captures")?;

    let cropped = image::imageops::crop_imm(
        &captured.image,
        selection.x,
        selection.y,
        selection.width,
        selection.height,
    )
    .to_image();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let filename = format!("shot-{}.png", timestamp);
    cropped
        .save(&filename)
        .with_context(|| format!("Failed to save screenshot to {filename}"))?;
    Ok(filename)
}

fn capture_all_monitors() -> Vec<ui::CapturedMonitor> {
    let monitors = Monitor::all().expect("failed to enumerate monitors");

    let mut snapshots = Vec::with_capacity(monitors.len());

    for monitor in monitors.into_iter() {
        let key = ui::MonitorKey {
            x: monitor.x().expect("monitor.x() failed"),
            y: monitor.y().expect("monitor.y() failed"),
            width: monitor.width().expect("monitor.width() failed"),
            height: monitor.height().expect("monitor.height() failed"),
        };

        let image = monitor.capture_image().expect("monitor.capture_image() failed");
        snapshots.push(ui::CapturedMonitor { key, image });
    }

    snapshots
}

fn main() -> eframe::Result {
    let all_captures = capture_all_monitors();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_fullscreen(true),
        ..Default::default()
    };

    eframe::run_native(
        "shotmd",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let capture_bytes: Arc<RwLock<Option<Vec<u8>>>> = Arc::new(RwLock::new(None));
            let bytes_loader = Arc::new(ShotmdBytesLoader::new(capture_bytes.clone()));
            cc.egui_ctx.add_bytes_loader(bytes_loader);

            Ok(Box::new(App {
                ui_state: ui::UiState::new(capture_bytes),
                all_captures,
            }))
        }),
    )
}

pub struct ShotmdBytesLoader {
    bytes: Arc<RwLock<Option<Vec<u8>>>>,
}

impl ShotmdBytesLoader {
    pub fn new(bytes: Arc<RwLock<Option<Vec<u8>>>>) -> Self {
        Self { bytes }
    }
}

impl BytesLoader for ShotmdBytesLoader {
    fn id(&self) -> &str {
        concat!(module_path!(), "::ShotmdBytesLoader")
    }

    fn load(&self, _ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if uri.starts_with("shotmd://capture") {
            if let Some(bytes) = self.bytes.read().unwrap().as_ref() {
                return Ok(BytesPoll::Ready {
                    size: None,
                    bytes: bytes.clone().into(),
                    mime: Some("image/png".to_string()),
                });
            }
        }
        Err(LoadError::NotSupported)
    }

    fn forget(&self, uri: &str) {
        let _ = uri;
    }

    fn forget_all(&self) {}

    fn byte_size(&self) -> usize {
        self.bytes
            .read()
            .unwrap()
            .as_ref()
            .map(|b| b.len())
            .unwrap_or(0)
    }
}