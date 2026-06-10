mod bytes_loader;

use shotmd::ui;
use shotmd::ui::Command;
use clap::Parser;
use std::sync::{Arc, Mutex, RwLock};
use xcap::Monitor;
use bytes_loader::ShotmdBytesLoader;
use shotmd::commands::{capture_command, record_command};

#[derive(Parser)]
#[command(name = "shotmd", version)]
struct Cli {
    #[arg(long)]
    full_screen: bool,
}

struct PendingAction {
    selection: ui::Selection,
    command: ui::Command,
}

struct App {
    ui_state: ui::UiState,
    all_captures: Arc<Vec<ui::CapturedMonitor>>,
    pending_action: Arc<Mutex<Option<PendingAction>>>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            return;
        };

        let mut result = None;
        egui::CentralPanel::no_frame().show_inside(ui, |ui| {
            result = self.ui_state.try_select(&*self.all_captures, window, ui);
        });

        // 'f' key shortcut to capture full screen of current monitor
        let f_pressed = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
        if f_pressed {
            let monitor_key = ui::monitor_key_for_window(&*self.all_captures, window);
            if let Some(captured) = self.all_captures.iter().find(|c| c.key == monitor_key) {
                let selection = ui::Selection {
                    x: 0,
                    y: 0,
                    width: captured.image.width(),
                    height: captured.image.height(),
                    monitor: monitor_key,
                };
                *self.pending_action.lock().unwrap() = Some(PendingAction {
                    selection,
                    command: Command::Capture,
                });
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        if let Some((selection, command)) = result {
            *self.pending_action.lock().unwrap() = Some(PendingAction {
                selection,
                command,
            });
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
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
            capture_command::capture_selection(&selection, &all_captures).expect("save_selection failed");
        }
        return Ok(());
    }

    let all_captures = Arc::new(capture_all_monitors());
    let pending_action: Arc<Mutex<Option<PendingAction>>> = Arc::new(Mutex::new(None));

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
            let pending = pending_action.clone();

            Ok(Box::new(App {
                ui_state: ui::UiState::new(capture_bytes),
                all_captures: all_captures.clone(),
                pending_action: pending,
            }))
        }),
    )?;

    if let Some(pending) = pending_action.lock().unwrap().take() {
        match pending.command {
            ui::Command::Capture => {
                capture_command::capture_selection(&pending.selection, &*all_captures)
                    .expect("save_selection failed");
            }
            ui::Command::Record { duration_secs } => {
                record_command::record_selection(&pending.selection, duration_secs).expect("record_selection failed");
            }
        }
    }

    Ok(())
}

