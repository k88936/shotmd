mod shotapp;

use egui::ViewportId;
use egui_wgpu::winit::Painter;
use egui_wgpu::{RendererOptions, WgpuConfiguration};
use egui_winit::State as EguiState;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Fullscreen, Window};
use shotapp::ShotApp;

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
            self.shot_app.ui(window, ui);
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

        let window = self.window.clone().unwrap();
        let size = window.inner_size();
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            self.painter
                .on_window_resized(viewport_id, width, height);
        }

        self.shot_app.sync_capture_to_window(&window, &self.egui_ctx);
        window.request_redraw();
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
            WindowEvent::Moved(_) => {
                self.shot_app.sync_capture_to_window(window, &self.egui_ctx);
                window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter
                        .on_window_resized(ViewportId::ROOT, width, height);
                }
                self.shot_app.sync_capture_to_window(window, &self.egui_ctx);
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
                self.shot_app.sync_capture_to_window(window, &self.egui_ctx);
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

}

fn main() -> Result<(), winit::error::EventLoopError> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)
}
