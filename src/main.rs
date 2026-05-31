mod ui;

use anyhow::Context;
use clap::Parser;
use egui::load::{BytesLoadResult, BytesLoader, BytesPoll, LoadError};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;

#[derive(Parser)]
#[command(name = "shotmd", version)]
struct Cli {
    #[arg(long)]
    full_screen: bool,
}

struct App {
    ui_state: ui::UiState,
    all_captures: Vec<ui::CapturedMonitor>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Capture,
    Record,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            return;
        };

        let mut result = None;
        egui::CentralPanel::no_frame().show_inside(ui, |ui| {
            result = self.ui_state.try_select(&self.all_captures, window, ui);
        });

        // 'f' key shortcut to capture full screen of current monitor
        let f_pressed = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
        if f_pressed {
            let monitor_key = ui::monitor_key_for_window(&self.all_captures, window);
            if let Some(captured) = self.all_captures.iter().find(|c| c.key == monitor_key) {
                let selection = ui::Selection {
                    x: 0,
                    y: 0,
                    width: captured.image.width(),
                    height: captured.image.height(),
                    monitor: monitor_key,
                };
                save_selection(&selection, &self.all_captures).expect("save_selection failed");
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        let command = Command::Capture;
        if let Some(selection) = result {
            match command {
                Command::Capture => {
                    save_selection(&selection, &self.all_captures).expect("save_selection failed");
                }
                Command::Record => todo!(),
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

        let image = monitor
            .capture_image()
            .expect("monitor.capture_image() failed");
        snapshots.push(ui::CapturedMonitor { key, image });
    }

    snapshots
}

fn main() -> eframe::Result {
    let cli = Cli::parse();

    if cli.full_screen {
        let all_captures = capture_all_monitors();
        for capture in &all_captures {
            let selection = ui::Selection {
                x: 0,
                y: 0,
                width: capture.image.width(),
                height: capture.image.height(),
                monitor: capture.key,
            };
            save_selection(&selection, &all_captures).expect("save_selection failed");
        }
        return Ok(());
    }

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
