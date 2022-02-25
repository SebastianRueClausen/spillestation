//! This module implements GUI used for debugging and more. It uses the egui crate to render it to
//! the screen.

pub mod app;
pub mod app_menu;
pub mod cpu;
pub mod fps;
pub mod gpu;
pub mod irq;
pub mod mem;
pub mod timer;
pub mod vram;
pub mod schedule;
pub mod start_menu;

mod keys;

use crate::render::{Renderer, SurfaceSize};

use egui::{ClippedMesh, CtxRef};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_winit::State as WinState;
use winit::window::Window;

pub use app::App;
/// All the egui stuff required to draw gui to the screen.
pub struct GuiCtx {
    pub egui_ctx: CtxRef,
    win_state: WinState,
    screen_descriptor: ScreenDescriptor,
    render_pass: RenderPass,
    jobs: Vec<ClippedMesh>,
}

impl GuiCtx {
    pub fn new(scale_factor: f32, renderer: &Renderer) -> Self {
        let egui_ctx = CtxRef::default();
        egui_ctx.set_visuals(visuals());
        let win_state = WinState::from_pixels_per_point(scale_factor);
        let SurfaceSize {
            width: physical_width,
            height: physical_height,
        } = renderer.surface_size;
        let screen_descriptor = ScreenDescriptor {
            physical_width,
            physical_height,
            scale_factor,
        };
        let render_pass = RenderPass::new(&renderer.device, renderer.surface_format, 1);
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

    /// Render the current frame to the screen.
    pub fn render<F>(
        &mut self,
        render_ctx: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        window: &Window,
        func: F,
    ) -> Result<(), BackendError>
    where
        F: FnOnce(&egui::CtxRef),
    {
        let input = self.win_state.take_egui_input(window);
        let (output, shapes) = self.egui_ctx.run(input, |ctx| {
            func(ctx);
        });
        self.render_pass.update_texture(
            &render_ctx.device,
            &render_ctx.queue,
            &self.egui_ctx.font_image(),
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
            .execute(encoder, target, &self.jobs, &self.screen_descriptor, None)?;
        self.win_state.handle_output(window, &self.egui_ctx, output);
        self.jobs = self.egui_ctx.tessellate(shapes);
        Ok(())
    }
}

fn visuals() -> egui::Visuals {
    egui::Visuals {
        widgets: widget_style(),
        selection: egui::style::Selection {
            bg_fill: PERSIAN_ACCENT,
            stroke: egui::Stroke {
                color: egui::Color32::from_rgb(229, 229, 229),
                width: 1.0,
            },
        },
        button_frame: true,
        collapsing_header_frame: true,
        faint_bg_color: egui::Color32::from_rgb(132, 132, 132),
        extreme_bg_color: egui::Color32::from_rgb(132, 132, 132),
        ..Default::default()
    }
}

fn widget_style() -> egui::style::Widgets {
    egui::style::Widgets {
        active: egui::style::WidgetVisuals {
            bg_fill: egui::Color32::from_rgb(229, 229, 229),
            bg_stroke: egui::Stroke {
                color: PERSIAN_ACCENT,
                width: 2.0,
            },
            corner_radius: 4.0,
            fg_stroke: egui::Stroke {
                color: egui::Color32::from_rgb(132, 132, 132),
                width: 2.0,
            },
            expansion: 1.0,
        },
        noninteractive: egui::style::WidgetVisuals {
            bg_fill: egui::Color32::from_rgb(187, 187, 187),
            bg_stroke: egui::Stroke {
                color: egui::Color32::from_rgb(132, 132, 132),
                width: 1.0,
            },
            corner_radius: 4.0,
            fg_stroke: egui::Stroke {
                color: egui::Color32::BLACK,
                width: 1.0,
            },
            expansion: 0.0,
        },
        hovered: egui::style::WidgetVisuals {
            bg_fill: egui::Color32::from_rgb(229, 229, 229),
            bg_stroke: egui::Stroke {
                color: PERSIAN_ACCENT,
                width: 2.0,
            },
            corner_radius: 4.0,
            fg_stroke: egui::Stroke {
                color: egui::Color32::BLACK,
                width: 1.0,
            },
            expansion: 0.0,
        },
        inactive: egui::style::WidgetVisuals {
            bg_fill: egui::Color32::from_rgb(229, 229, 229),
            bg_stroke: egui::Stroke {
                color: egui::Color32::from_rgb(132, 132, 132),
                width: 1.0,
            },
            corner_radius: 4.0,
            fg_stroke: egui::Stroke {
                color: egui::Color32::BLACK,
                width: 1.0,
            },
            expansion: 0.0,
        },
        open: egui::style::WidgetVisuals {
            bg_fill: PERSIAN_ACCENT,
            bg_stroke: egui::Stroke {
                color: PERSIAN_ACCENT,
                width: 1.0,
            },
            corner_radius: 4.0,
            fg_stroke: egui::Stroke {
                color: egui::Color32::BLACK,
                width: 1.0,
            },
            expansion: 0.0,
        },
    }
}

pub const PERSIAN_ACCENT: egui::Color32 = egui::Color32::from_rgb(1, 172, 159);
// pub const RED_ACCENT: egui::Color32 = egui::Color32::from_rgb(223, 0, 36);
// const BLUE_ACCENT: egui::Color32 = egui::Color32::from_rgb(46, 109, 180);
// const YELLOW_ACCENT: egui::Color32 = egui::Color32::from_rgb(243, 195, 0);
