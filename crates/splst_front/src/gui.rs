//! This module implements GUI used for debugging and more. It uses the egui crate to render it to
//! the screen.

use splst_render::{Renderer, SurfaceSize};

use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_winit::State as WinState;
use winit::window::Window;

use std::sync::Arc;

struct ErrorPopup {
    heading: String,
    text: String,
    id: egui::Id,
}

#[derive(Default)]
pub struct GuiContext {
    pub egui_ctx: egui::Context,
    /// Each new [`ErrorPopup`] get's assigned a unique ID, which is derived from the total amount
    /// of errors shown.
    error_count: usize,
    errors: Vec<ErrorPopup>,
    main_style: Arc<egui::Style>,
}

impl GuiContext {
    pub fn error(&mut self, heading: impl Into<String>, text: impl Into<String>) {
        self.errors.push(ErrorPopup {
            heading: heading.into(),
            text: text.into(),
            // This is very likely not necessary to be a string, but it assures that we don't
            // get any collisions.
            id: egui::Id::new(format!("Error {}", self.error_count)),
        });
        self.error_count += 1;
    }

    fn show_errors(&mut self) {
        self.errors.retain(|error| {
            let mut open = true;
            egui::Window::new(&error.heading)
                .open(&mut open)
                .id(error.id)
                .resizable(false)
                .collapsible(false)
                .show(&self.egui_ctx, |ui| {
                    ui.label(&error.text);
                });
            open 
        });
    }
}

/// All the egui stuff required to draw gui to the screen.
pub struct GuiRenderer {
    ctx: GuiContext,
    win_state: WinState,
    screen_descriptor: ScreenDescriptor,
    render_pass: RenderPass,
    jobs: Vec<egui::ClippedMesh>,
    textures: egui::TexturesDelta,
}

impl GuiRenderer {
    pub fn ctx(&mut self) -> &mut GuiContext {
        &mut self.ctx
    }

    pub fn new(scale_factor: f32, renderer: &Renderer) -> Self {
        let ctx = GuiContext::default();

        ctx.egui_ctx.set_style(ctx.main_style.clone());

        let max_texture_dim = renderer.device.limits().max_texture_dimension_2d as usize;
        let win_state = WinState::from_pixels_per_point(max_texture_dim, scale_factor);

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
            ctx,
            win_state,
            screen_descriptor,
            render_pass,
            jobs: Vec::new(),
            textures: egui::TexturesDelta::default(),
        }
    }

    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) {
        self.win_state.on_event(&self.ctx.egui_ctx, event);
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
        renderer: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        window: &Window,
        func: F,
    ) -> Result<(), BackendError>
    where
        F: FnOnce(&mut GuiContext),
    {
        let input = self.win_state.take_egui_input(window);
        let output = self.ctx.egui_ctx.clone().run(input, |_| {
            self.ctx.show_errors();
            func(&mut self.ctx)
        });

        self.textures.append(output.textures_delta);
        self.win_state
            .handle_platform_output(window, &self.ctx.egui_ctx, output.platform_output);

        self.jobs = self.ctx.egui_ctx.tessellate(output.shapes);

        self.render_pass
            .add_textures(&renderer.device, &renderer.queue, &self.textures)?;

        self.render_pass.update_buffers(
            &renderer.device,
            &renderer.queue,
            &self.jobs,
            &self.screen_descriptor,
        );

        self.render_pass
            .execute(encoder, target, &self.jobs, &self.screen_descriptor, None)?;

        let textures = std::mem::take(&mut self.textures);
        self.render_pass.remove_textures(textures)
    }
}

/*
fn main_visuals() -> egui::Visuals {
    egui::Visuals {
        widgets: main_widget_style(),
        selection: egui::style::Selection {
            bg_fill: BLUE_ACCENT,
            stroke: egui::Stroke {
                color: FOREGROUND,
                width: 1.0,
            },
        },
        button_frame: true,
        collapsing_header_frame: true,
        faint_bg_color: WHITE_ACCENT,
        extreme_bg_color: WHITE_FOREGROUND,
        window_rounding: egui::Rounding {
            nw: 10.0,
            ne: 10.0,
            sw: 10.0,
            se: 10.0
        },
        ..Default::default()
    }
}

fn main_widget_style() -> egui::style::Widgets {
    egui::style::Widgets {
        active: egui::style::WidgetVisuals {
            bg_fill: BLUE_ACCENT,
            rounding: CORNER_ROUNDING,
            bg_stroke: egui::Stroke {
                color: BACKGROUND,
                width: 1.0,
            },
            fg_stroke: egui::Stroke {
                color: FOREGROUND,
                width: 2.0,
            },
            expansion: 0.0,
        },
        noninteractive: egui::style::WidgetVisuals {
            bg_fill: BACKGROUND,
            rounding: CORNER_ROUNDING,
            bg_stroke: egui::Stroke {
                color: BACKGROUND,
                width: 1.0,
            },
            fg_stroke: egui::Stroke {
                color: FOREGROUND,
                width: 1.0,
            },
            expansion: 0.0,
        },
        hovered: egui::style::WidgetVisuals {
            bg_fill: WHITE_FOREGROUND,
            rounding: CORNER_ROUNDING,
            bg_stroke: egui::Stroke {
                color: BACKGROUND,
                width: 1.0,
            },
            fg_stroke: egui::Stroke {
                color: FOREGROUND,
                width: 1.0,
            },
            expansion: 0.0,
        },
        inactive: egui::style::WidgetVisuals {
            bg_fill: WHITE_FOREGROUND,
            rounding: CORNER_ROUNDING,
            bg_stroke: egui::Stroke {
                color: BACKGROUND,
                width: 1.0,
            },
            fg_stroke: egui::Stroke {
                color: FOREGROUND,
                width: 1.0,
            },
            expansion: 0.0,
        },
        open: egui::style::WidgetVisuals {
            bg_fill: BLUE_ACCENT,
            rounding: CORNER_ROUNDING,
            bg_stroke: egui::Stroke {
                color: BACKGROUND,
                width: 1.0,
            },
            fg_stroke: egui::Stroke {
                color: FOREGROUND,
                width: 1.0,
            },
            expansion: 0.0,
        },
    }
}

const CORNER_ROUNDING: egui::Rounding = egui::Rounding {
    nw: 6.0,
    ne: 6.0,
    sw: 6.0,
    se: 6.0,
};

const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(209, 207, 205);
const FOREGROUND: egui::Color32 = egui::Color32::from_rgb(13, 22, 29);
const WHITE_FOREGROUND: egui::Color32 = egui::Color32::from_rgb(240, 240, 240);
const WHITE_ACCENT: egui::Color32 = egui::Color32::from_rgb(199, 199, 199);
const BLUE_ACCENT: egui::Color32 = egui::Color32::from_rgb(73, 145, 163);
*/
