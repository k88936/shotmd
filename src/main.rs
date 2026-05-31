mod ui;

use anyhow::Context;
use clap::Parser;
use egui::load::{BytesLoadResult, BytesLoader, BytesPoll, LoadError};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;

#[derive(Parser)]
#[command(name = "shotmd", version)]
struct Cli {
    #[arg(long)]
    full_screen: bool,
}

struct PendingAction {
    selection: ui::Selection,
    command: ui::Command,
    all_captures: Vec<ui::CapturedMonitor>,
}

struct App {
    ui_state: ui::UiState,
    all_captures: Vec<ui::CapturedMonitor>,
    pending_action: Arc<Mutex<Option<PendingAction>>>,
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

        if let Some((selection, command)) = result {
            *self.pending_action.lock().unwrap() = Some(PendingAction {
                selection,
                command,
                all_captures: self.all_captures.clone(),
            });
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

fn record_selection(selection: &ui::Selection, duration_secs: u64) -> anyhow::Result<String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::RecvTimeoutError;
    use std::thread;
    use std::time::Duration;

    let monitor = Monitor::from_point(selection.monitor.x, selection.monitor.y)
        .context("Failed to get monitor from point")?;

    let (video_recorder, rx) = monitor
        .video_recorder()
        .context("Failed to create video recorder")?;

    let frames = Arc::new(Mutex::new(Vec::new()));
    let frames_clone = frames.clone();

    let recording_done = Arc::new(AtomicBool::new(false));
    let recording_done_clone = recording_done.clone();

    let collector = thread::spawn(move || {
        let mut collected = frames_clone.lock().unwrap();
        loop {
            if recording_done_clone.load(Ordering::SeqCst) {
                // Drain any remaining frames already buffered in the channel
                while let Ok(frame) = rx.try_recv() {
                    collected.push(frame);
                }
                break;
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(frame) => collected.push(frame),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    video_recorder
        .start()
        .context("Failed to start recording")?;
    thread::sleep(Duration::from_secs(duration_secs));
    video_recorder.stop().context("Failed to stop recording")?;

    // Signal the collector to stop. xcap's internal background thread holds
    // a cloned sender even after stop(), so the channel never closes naturally.
    recording_done.store(true, Ordering::SeqCst);
    collector.join().expect("collector thread panicked");

    let frames = frames.lock().unwrap();
    let frame_count = frames.len();
    if frame_count == 0 {
        anyhow::bail!("No frames captured");
    }

    let delay_ms = ((duration_secs * 1000) as u32) / (frame_count as u32);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let filename = format!("shot-{}.gif", timestamp);

    let file =
        std::fs::File::create(&filename).with_context(|| format!("Failed to create {filename}"))?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .context("Failed to set repeat")?;

    for frame in frames.iter() {
        let rgba = image::RgbaImage::from_raw(frame.width, frame.height, frame.raw.clone())
            .context("Failed to create RgbaImage from frame")?;
        let cropped = image::imageops::crop_imm(
            &rgba,
            selection.x,
            selection.y,
            selection.width,
            selection.height,
        )
        .to_image();
        let gif_frame = image::Frame::from_parts(
            cropped,
            0,
            0,
            image::Delay::from_numer_denom_ms(delay_ms, 1),
        );
        encoder
            .encode_frame(gif_frame)
            .context("Failed to encode GIF frame")?;
    }

    Ok(filename)
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
                all_captures,
                pending_action: pending,
            }))
        }),
    )?;

    if let Some(pending) = pending_action.lock().unwrap().take() {
        match pending.command {
            ui::Command::Capture => {
                save_selection(&pending.selection, &pending.all_captures)
                    .expect("save_selection failed");
            }
            ui::Command::Record { duration_secs } => {
                match record_selection(&pending.selection, duration_secs) {
                    Ok(filename) => println!("Recording saved to {filename}"),
                    Err(e) => eprintln!("Recording failed: {e}"),
                }
            }
        }
    }

    Ok(())
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
