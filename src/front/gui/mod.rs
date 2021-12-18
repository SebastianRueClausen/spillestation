//! This module implements GUI used for debugging and more. It uses the egui crate to render it to
//! the screen.

pub mod app;
pub mod app_menu;
pub mod cpu;
pub mod fps;
pub mod gpu;
pub mod mem;
pub mod vram;
pub mod config;

use super::{RenderCtx, SurfaceSize};
pub use app::App;
use egui::{ClippedMesh, CtxRef};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_winit::State as WinState;
use winit::window::Window;

/// All the egui stuff required to draw gui to the screen.
pub struct GuiCtx {
    /// The main egui context.
    pub egui_ctx: CtxRef,
    /// The ['egui_winit'] context.
    win_state: WinState,
    /// Screen descriptor used by ['egui_wgpu_backend'].
    screen_descriptor: ScreenDescriptor,
    /// Render pass used by ['egui_wgpu_backend'].
    render_pass: RenderPass,
    jobs: Vec<ClippedMesh>,
}

impl GuiCtx {
    pub fn new(scale_factor: f32, render_ctx: &RenderCtx) -> Self {
        let egui_ctx = CtxRef::default();
        let win_state = WinState::from_pixels_per_point(scale_factor);
        let SurfaceSize {
            width: physical_width,
            height: physical_height,
        } = render_ctx.surface_size;
        let screen_descriptor = ScreenDescriptor {
            physical_width,
            physical_height,
            scale_factor,
        };
        let render_pass = RenderPass::new(&render_ctx.device, render_ctx.surface_format, 1);
        Self {
            egui_ctx,
            win_state,
            screen_descriptor,
            render_pass,
            jobs: Vec::new(),
        }
    }

    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) {
        self.win_state.on_event(&self.egui_ctx, event);
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.screen_descriptor.scale_factor = scale_factor;
    }

    pub fn resize(&mut self, size: SurfaceSize) {
        // Only do something if it isn't being minimized.
        if size.height > 0 && size.width > 0 {
            self.screen_descriptor.physical_width = size.width;
            self.screen_descriptor.physical_height = size.height;
        }
    }

    pub fn show_app<T: App>(&mut self, app: &mut T) -> bool {
        let mut open = true;
        app.show(&self.egui_ctx, &mut open);
        open
    }

    /// Prepare egui to take commands.
    pub fn begin_frame(&mut self, window: &Window) {
        let input = self.win_state.take_egui_input(window);
        self.egui_ctx.begin_frame(input);
    }

    /// Prepare egui to render all the windows.
    pub fn end_frame(&mut self, window: &Window) {
        let (output, jobs) = self.egui_ctx.end_frame();
        self.win_state.handle_output(window, &self.egui_ctx, output);
        self.jobs = self.egui_ctx.tessellate(jobs);
    }

    /// Render the current frame to the screen.
    pub fn render(
        &mut self,
        render_ctx: &RenderCtx,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) -> Result<(), BackendError> {
        self.render_pass.update_texture(
            &render_ctx.device,
            &render_ctx.queue,
            &self.egui_ctx.texture(),
        );
        self.render_pass
            .update_user_textures(&render_ctx.device, &render_ctx.queue);
        self.render_pass.update_buffers(
            &render_ctx.device,
            &render_ctx.queue,
            &self.jobs,
            &self.screen_descriptor,
        );
        self.render_pass
            .execute(encoder, target, &self.jobs, &self.screen_descriptor, None)
    }
}
