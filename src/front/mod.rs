//! The frontend of the emulator. Handles the window, input/output, rendering, GUI and controls the
//! emulator itself.

mod config;
mod gui;

pub mod render;

use crate::{system::System, bus::bios::Bios, timing};

use config::Config;
use gui::{App, app_menu::AppMenu, GuiCtx, config::Configurator};
use render::{Canvas, ComputeStage, DrawStage, RenderCtx, SurfaceSize};

use std::path::Path;
use std::time::{Duration, Instant};

use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Window};

pub use render::compute::DrawInfo;

/// The different ways the emulator can run.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Debug mode allows for stepping through each cycle, running at different speeds, and
    /// debug features such as breakpoints.
    Debug,
    /// Emulation runs the emulator at native speed.
    Emulation,
}

/// The stage of ['Frontend'].
enum Stage {
    /// It tries to load a valid config file, if that succeeds, then the
    /// state get's changed directly to ['Running']. If not, the stage get's changed to ['Config'].
    Startup,
    /// The user is presented with a GUI window to select a valid BIOS file(and other configs in
    /// the future). The stage get's changed to ['Running'] if a valid BIOS file can be loaded.
    Config {
        configurator: Configurator,
        bios: Option<Bios>,
    },
    /// The main stage where the emulator is running.
    Running {
        system: System,
        app_menu: Box<AppMenu>,
        /// The last time 'system' ran.
        last_update: Instant,
        /// How to run 'system'.
        mode: RunMode,
    },
}

/// The frontend of the emulator. It handles the window, input/output, config,
/// running the emulator and rendering the output of the playstation to.
pub struct Frontend {
    stage: Stage,
    render_ctx: RenderCtx,
    gui_ctx: GuiCtx,
    canvas: Canvas,
    comp_pass: ComputeStage,
    draw_pass: DrawStage,
    event_loop: EventLoop<()>,
    window: Window,
    /// When the last frame was drawn.
    last_draw: Instant,
    /// How long each frame should last. This depends on the systems video mode ie. if it's NTSC or
    /// PAL.
    frame_time: Duration,
}

impl Frontend {
    pub fn run(mut self) {
        self.event_loop.run(move |event, _, ctrl_flow| {
            *ctrl_flow = ControlFlow::Poll;
            match event {
                Event::RedrawEventsCleared => {
                    let dt = self.last_draw.elapsed();
                    let mut redraw = || {
                        self.window.request_redraw();
                        self.last_draw = Instant::now();
                    };
                    match self.stage {
                        Stage::Running {
                            ref mut app_menu,
                            ref system,
                            mode,
                            ..
                        } => match mode {
                            RunMode::Emulation => {
                                if system.cpu.bus().gpu().in_vblank() {
                                    redraw();
                                }
                            }
                            RunMode::Debug => {
                                if dt >= self.frame_time {
                                    app_menu.draw_tick(dt);
                                    redraw();
                                }
                            }
                        },
                        Stage::Config { .. } if dt >= self.frame_time => redraw(),
                        _ => {
                            *ctrl_flow = ControlFlow::WaitUntil(
                                Instant::now() + self.frame_time - dt,
                            );
                        }
                    }
                }
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == self.window.id() => {
                    match event {
                        WindowEvent::CloseRequested => *ctrl_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            let size = SurfaceSize {
                                width: physical_size.width,
                                height: physical_size.height,
                            };
                            self.render_ctx.resize(size);
                            self.draw_pass.resize(&self.render_ctx, &self.canvas);
                            self.gui_ctx.resize(size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            self.render_ctx.resize(SurfaceSize::new(
                                new_inner_size.width,
                                new_inner_size.height,
                            ));
                            self.draw_pass.resize(&self.render_ctx, &self.canvas);
                            self.gui_ctx.set_scale_factor(self.window.scale_factor() as f32);
                        }
                        // Handle keyboard input.
                        WindowEvent::KeyboardInput { input, .. } => {
                            if let Stage::Running { ref mut app_menu, .. } = self.stage {
                                if let (Some(VirtualKeyCode::Escape), ElementState::Pressed)
                                    = (input.virtual_keycode, input.state)
                                {
                                    app_menu.toggle_open(); 
                                }
                            }
                            self.gui_ctx.handle_window_event(event);
                        }
                        _ => {
                            self.gui_ctx.handle_window_event(event);
                        }
                    }
                }
                Event::RedrawRequested(_) => match self.stage {
                    Stage::Running {
                        ref mut system,
                        ref mut app_menu,
                        ref mut mode,
                        ..
                    } => {
                        self.render_ctx.render(|encoder, view, renderer| {
                            self.comp_pass.compute_canvas(
                                system.cpu.bus().vram(),
                                &system.cpu.bus().gpu().draw_info(),
                                encoder,
                                &renderer.queue,
                                &self.canvas,
                            );
                            self.draw_pass.render_canvas(encoder, view);
                            let res = self.gui_ctx.render(renderer, encoder, view, &self.window, |gui| {
                                app_menu.show(gui, mode);
                            });
                            if let Err(ref err) = res {
                                app_menu.close_apps();
                                error!("Failed to render GUI: {}", err);
                            }
                        });
                    }
                    Stage::Config { ref mut configurator, ref mut bios } => {
                        let mut config_open = true;
                        self.render_ctx.render(|encoder, view, renderer| {
                            self.draw_pass.render_canvas(encoder, view);
                            let res = self.gui_ctx.render(renderer, encoder, view, &self.window, |gui| {
                                configurator.show_window(gui, &mut config_open);
                            });
                            if let Err(ref err) = res {
                                error!("Failed to render GUI: {}", err);
                            }
                        });
                        if !config_open {
                            match bios.take() {
                                Some(bios) => {
                                    if let Err(err) = configurator.config.store() {
                                        error!("Failed to store config file: {}", err)
                                    }
                                    self.stage = Stage::Running {
                                        system: System::new(bios),
                                        app_menu: Box::new(AppMenu::new()),
                                        last_update: Instant::now(),
                                        mode: RunMode::Emulation,
                                    };
                                },
                                None => {
                                    warn!("Tried to load config, without BIOS file");
                                    configurator.bios_message = Some(
                                        String::from("A BIOS file must be loaded")
                                    );
                                },
                            }
                        }
                    }
                    Stage::Startup => {}
                },
                Event::MainEventsCleared => match self.stage {
                    Stage::Running {
                        ref mut system,
                        ref mut app_menu,
                        ref mut last_update,
                        mode,
                    } => {
                        match mode {
                            RunMode::Debug => {
                                app_menu.update_tick(last_update.elapsed(), system);
                            }
                            RunMode::Emulation => {
                                system.run(last_update.elapsed());
                            }
                        }
                        *last_update = Instant::now();
                    },
                    Stage::Config {
                        ref mut configurator,
                        ref mut bios,
                    } if configurator.try_load_bios => {
                        match Bios::from_file(Path::new(&configurator.config.bios)) {
                            Err(ref err) => {
                                configurator.try_load_bios = false;
                                configurator.bios_message = Some(format!("{}", err));    
                            },
                            Ok(loaded_bios) => {
                                *bios = Some(loaded_bios);
                                configurator.bios_message = Some(
                                    String::from("BIOS loaded successfully")
                                );
                                configurator.try_load_bios = false;
                            },
                        };
                    }
                    Stage::Startup => {
                        self.stage = match Config::load() {
                            Ok(config) => match Bios::from_file(Path::new(&config.bios)) {
                                Err(ref err) => Stage::Config {
                                    configurator: Configurator::new(
                                        None,
                                        Some(format!("{}", err)),
                                    ),
                                    bios: None,
                                },
                                Ok(bios) => Stage::Running {
                                    system: System::new(bios),
                                    app_menu: Box::new(AppMenu::new()),
                                    last_update: Instant::now(),
                                    mode: RunMode::Emulation,
                                },
                            },
                            Err(ref err) => {
                                Stage::Config {
                                    configurator: Configurator::new(
                                        Some(format!("{}", err)),
                                        None,
                                    ),
                                    bios: None,
                                }
                            },
                        };
                    }
                    _ => {},
                },
                _ => {},
            }
        });

    }

    pub fn new() -> Self {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("spillestation")
            .build(&event_loop)
            .expect("Failed to create window");
        let render_ctx = RenderCtx::new(&window);
        let canvas = Canvas::new(&render_ctx.device, SurfaceSize::new(640, 480));
        let frame_time = Duration::from_secs_f32(1.0 / timing::NTSC_FPS as f32);
        Self {
            stage: Stage::Startup,
            gui_ctx: GuiCtx::new(window.scale_factor() as f32, &render_ctx),
            comp_pass: ComputeStage::new(&render_ctx.device, &canvas),
            draw_pass: DrawStage::new(&render_ctx, &canvas),
            last_draw: Instant::now(),
            render_ctx,
            canvas,
            event_loop,
            window,
            frame_time,
        }
    }
}
