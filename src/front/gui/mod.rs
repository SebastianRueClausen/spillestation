use egui::{ClippedMesh,CtxRef};
use egui_wgpu_backend::{BackendError,RenderPass,ScreenDescriptor};
use egui_winit::State as WinState;
use winit::window::Window;
use super::{SurfaceSize,RenderCtx};
use app::App;

pub mod fps;
pub mod cpu;
pub mod app;
pub mod mem;

/// All the egui stuff required to draw gui to the screen.
pub struct GuiCtx {
    ctx: CtxRef,
    win_state: WinState,
    screen_descriptor: ScreenDescriptor,
    render_pass: RenderPass,
    jobs: Vec<ClippedMesh>,
}

impl GuiCtx {
    pub fn new(
        scale_factor: f32,
        render_ctx: &RenderCtx,
    ) -> Self {
        let ctx = CtxRef::default();
        let win_state = WinState::from_pixels_per_point(scale_factor);
        let SurfaceSize {
            width: physical_width,
            height: physical_height
        } = render_ctx.surface_size;
        let screen_descriptor = ScreenDescriptor {
            physical_width,
            physical_height,
            scale_factor,
        };
        let render_pass = RenderPass::new(
            &render_ctx.device,
            render_ctx.surface_format,
            1,
        );
        Self {
            ctx,
            win_state,
            screen_descriptor,
            render_pass,
            jobs: Vec::new(),
        }
    }

    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) {
        self.win_state.on_event(&self.ctx, event);
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

    /// Show an app. Must be called between 'begin_frame' and 'end_frame'.
    pub fn show_app<T: App>(&self, app: &mut T) {
        let mut _open: bool = true;
        app.show(&self.ctx, &mut _open);
    }

    /// Prepare egui to take commands.
    pub fn begin_frame(&mut self, window: &Window) {
       let input = self.win_state.take_egui_input(window); 
       self.ctx.begin_frame(input);
    }

    /// Prepare egui to render all the windows.
    pub fn end_frame(&mut self, window: &Window) {
       let (output, jobs) = self.ctx.end_frame();
       self.win_state.handle_output(window, &self.ctx, output);
       self.jobs = self.ctx.tessellate(jobs);
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), BackendError> {
        self.render_pass.update_texture(
            device,
            queue,
            &self.ctx.texture(),
        );
        self.render_pass.update_user_textures(device, queue);
        self.render_pass.update_buffers(
            device,
            queue,
            &self.jobs,
            &self.screen_descriptor,
        );
        self.render_pass.execute(
            encoder,
            target,
            &self.jobs,
            &self.screen_descriptor,
            None,
        )
    }
}

