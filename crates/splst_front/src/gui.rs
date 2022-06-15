//! This module implements GUI used for debugging and more. It uses the egui crate to render it to
//! the screen.

use splst_render::{Renderer, SurfaceSize};

use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_winit::State as WinState;
use winit::window::Window;

use core::hash::Hash;

struct Popup {
    heading: String,
    text: String,
}

pub struct Popups {
    popups: Vec<Popup>,
    /// Since there may be more `Popups`, we each one must have an unique `egui::Id`.
    base_id: egui::Id,
}

impl Popups {
    pub fn new(base_id: impl Hash) -> Self {
        Self {
            popups: Vec::default(),
            base_id: egui::Id::new(base_id),
        }
    }

    pub fn add(&mut self, heading: impl Into<String>, text: impl Into<String>) {
        self.popups.push(Popup {
            heading: heading.into(),
            text: text.into(),
        });
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let mut i = 0;

        self.popups.retain(|popup| {
            let mut open = true;

            egui::Window::new(&popup.heading)
                .open(&mut open)
                .id(egui::Id::new(i).with(self.base_id))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(&popup.text);
                });

            i += 1;

            open 
        });
    }
}

/// All the egui stuff required to draw gui to the screen.
pub struct GuiRenderer {
    pub ctx: egui::Context,
    pub popups: Popups,
    win_state: WinState,
    screen_descriptor: ScreenDescriptor,
    render_pass: RenderPass,
    jobs: Vec<egui::ClippedMesh>,
    textures: egui::TexturesDelta,
}

impl GuiRenderer {
    pub fn new(scale_factor: f32, renderer: &Renderer) -> Self {
        let ctx = egui::Context::default();

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
            popups: Popups::new("default"),
            win_state,
            screen_descriptor,
            render_pass,
            jobs: Vec::new(),
            textures: egui::TexturesDelta::default(),
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
        F: FnOnce(&egui::Context, &mut Popups),
    {
        let input = self.win_state.take_egui_input(window);
        let output = self.ctx.run(input, |ctx| {
            self.popups.show(ctx);
            func(&self.ctx, &mut self.popups)
        });

        self.textures.append(output.textures_delta);
        self.win_state
            .handle_platform_output(window, &self.ctx, output.platform_output);

        self.jobs = self.ctx.tessellate(output.shapes);

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
