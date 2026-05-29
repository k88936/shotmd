use egui::ViewportId;
use egui_wgpu::winit::Painter;
use egui_wgpu::{RendererOptions, WgpuConfiguration};
use egui_winit::State as EguiState;
use image::RgbaImage;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window};
use xcap::Monitor;

#[derive(Clone, Copy, Debug)]
struct Selection {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

struct ShotApp {
    monitors: Vec<Monitor>,
    monitor_labels: Vec<String>,
    selected_monitor: usize,
    texture: Option<egui::TextureHandle>,
    last_capture: Option<RgbaImage>,
    selection: Option<Selection>,
    selection_start: Option<egui::Pos2>,
    selection_end: Option<egui::Pos2>,
    last_saved: Option<String>,
    last_error: Option<String>,
}

impl ShotApp {
    fn new() -> Self {
        let mut app = Self {
            monitors: Vec::new(),
            monitor_labels: Vec::new(),
            selected_monitor: 0,
            texture: None,
            last_capture: None,
            selection: None,
            selection_start: None,
            selection_end: None,
            last_saved: None,
            last_error: None,
        };
        app.refresh_monitors();
        app
    }

    fn refresh_monitors(&mut self) {
        match Monitor::all() {
            Ok(monitors) => {
                self.monitor_labels = monitors
                    .iter()
                    .enumerate()
                    .map(|(index, monitor)| {
                        monitor
                            .friendly_name()
                            .or_else(|_| monitor.name())
                            .unwrap_or_else(|_| format!("Monitor {}", index + 1))
                    })
                    .collect();
                self.monitors = monitors;
                if self.selected_monitor >= self.monitors.len() {
                    self.selected_monitor = 0;
                }
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(format!("Failed to list monitors: {err}"));
                self.monitors.clear();
                self.monitor_labels.clear();
                self.selected_monitor = 0;
            }
        }
    }

    fn capture_selected(&mut self, ctx: &egui::Context) {
        let monitor = match self.monitors.get(self.selected_monitor) {
            Some(monitor) => monitor,
            None => {
                self.last_error = Some("No monitor available".to_string());
                return;
            }
        };

        match monitor.capture_image() {
            Ok(image) => {
                let size = [image.width() as usize, image.height() as usize];
                let rgba = image.clone().into_raw();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                match self.texture.as_mut() {
                    Some(texture) => {
                        texture.set(color_image, egui::TextureOptions::LINEAR);
                    }
                    None => {
                        self.texture = Some(ctx.load_texture(
                            "screenshot",
                            color_image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                self.last_capture = Some(image);
                self.selection = None;
                self.selection_start = None;
                self.selection_end = None;
                self.last_saved = None;
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(format!("Capture failed: {err}"));
            }
        }
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

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("controls").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let capture_enabled = !self.monitors.is_empty();
                if ui.add_enabled(capture_enabled, egui::Button::new("Capture")).clicked() {
                    self.capture_selected(ui.ctx());
                }
                ui.add_enabled_ui(capture_enabled, |ui| {
                    egui::ComboBox::from_id_salt("monitor_select")
                        .selected_text(
                            self.monitor_labels
                                .get(self.selected_monitor)
                                .cloned()
                                .unwrap_or_else(|| "No monitor".to_string()),
                        )
                        .show_ui(ui, |ui| {
                            for (index, label) in self.monitor_labels.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_monitor, index, label);
                            }
                        });
                });

                if ui.button("Refresh monitors").clicked() {
                    self.refresh_monitors();
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

struct App {
    shot_app: ShotApp,
    egui_ctx: egui::Context,
    egui_state: Option<EguiState>,
    painter: Painter,
    window: Option<Arc<Window>>,
    next_repaint: Option<Instant>,
    is_init: bool,
}

impl App {
    fn new() -> Self {
        let egui_ctx = egui::Context::default();
        let painter = pollster::block_on(Painter::new(
            egui_ctx.clone(),
            WgpuConfiguration::default(),
            false,
            RendererOptions::default(),
        ));
        Self {
            shot_app: ShotApp::new(),
            egui_ctx,
            egui_state: None,
            painter,
            window: None,
            next_repaint: None,
            is_init: true,
        }
    }

    fn render(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let Some(egui_state) = &mut self.egui_state else {
            return;
        };

        {
            let viewport_info = egui_state
                .egui_input_mut()
                .viewports
                .entry(ViewportId::ROOT)
                .or_default();
            egui_winit::update_viewport_info(&mut *viewport_info, &self.egui_ctx, window, self.is_init);
            self.is_init = false;
        }

        let raw_input = egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            self.shot_app.ui(ui);
        });
        egui_state.handle_platform_output(window, full_output.platform_output);

        let clipped_primitives =
            self.egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);

        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            full_output.pixels_per_point,
            [0.0, 0.0, 0.0, 1.0],
            &clipped_primitives,
            &full_output.textures_delta,
            Vec::new(),
        );

        let repaint_delay = full_output
            .viewport_output
            .get(&ViewportId::ROOT)
            .map(|output| output.repaint_delay)
            .unwrap_or(Duration::from_millis(16));
        if repaint_delay.is_zero() {
            window.request_redraw();
            self.next_repaint = None;
        } else {
            self.next_repaint = Instant::now().checked_add(repaint_delay);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("shotmd")
            .with_decorations(false)
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
        ;


        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Failed to create window"),
        );

        let viewport_id = ViewportId::ROOT;
        let egui_state = EguiState::new(
            self.egui_ctx.clone(),
            viewport_id,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        self.egui_state = Some(egui_state);
        self.window = Some(window.clone());

        pollster::block_on(self.painter.set_window(viewport_id, Some(window)))
            .expect("Failed to initialize egui wgpu painter");

        let size = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .unwrap_or_default();
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            self.painter
                .on_window_resized(viewport_id, width, height);
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        if let Some(egui_state) = &mut self.egui_state {
            let response = egui_state.on_window_event(window, &event);
            if response.repaint {
                window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter
                        .on_window_resized(ViewportId::ROOT, width, height);
                }
                window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = window.inner_size();
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter
                        .on_window_resized(ViewportId::ROOT, width, height);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = &self.window else {
            return;
        };
        if let Some(next_repaint) = self.next_repaint {
            let now = Instant::now();
            if now >= next_repaint {
                window.request_redraw();
                self.next_repaint = None;
                event_loop.set_control_flow(ControlFlow::Poll);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next_repaint));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

fn main() -> Result<(), winit::error::EventLoopError> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)
}
