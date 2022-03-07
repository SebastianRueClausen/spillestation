#![feature(vec_retain_mut)]

//! The frontend of the emulator. Handles the window, input/output, rendering, GUI and controls the
//! emulator itself.
//!
//! TODO:
//! - Find some way to make the emulator sleep in emulation mode if we are keeping up. Right now it
//!   will just continue to run for very small intervals all the time.

#[macro_use]
extern crate log;

mod gui;
mod start_menu;
mod render;

mod debug;
mod config;
mod keys;

use splst_core::{System, timing};
use crate::render::{Renderer, SurfaceSize};
use gui::GuiCtx;
use start_menu::StartMenu;
use debug::DebugMenu;
use config::Config;

use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Window};

use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Debug mode allows for stepping through each cycle, running at different speeds, and
    /// debug features such as breakpoints.
    Debug,
    /// Emulation runs the emulator at native speed.
    Emulation,
}

enum Stage {
    StartMenu(StartMenu),
    Running {
        system: System,
        app_menu: Box<DebugMenu>,
        /// The last time 'system' ran.
        last_update: Instant,
        mode: RunMode,
        show_settings: bool,
    },
}

/// The frontend of the emulator. It handles the window, input/output, config,
/// running the emulator and rendering the output of the playstation to.
pub struct Frontend {
    stage: Stage,
    config: Config,
    gui_ctx: GuiCtx,
    renderer: Renderer,
    event_loop: EventLoop<()>,
    window: Window,
    /// When the last frame was drawn.
    last_draw: Instant,
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
                        Stage::StartMenu(..) => {
                            if dt >= self.frame_time {
                                redraw();
                            }
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
                            self.renderer.resize(size);
                            self.gui_ctx.resize(size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            self.renderer.resize(SurfaceSize::new(
                                new_inner_size.width,
                                new_inner_size.height,
                            ));
                            self.gui_ctx.set_scale_factor(self.window.scale_factor() as f32);
                        }
                        // Handle keyboard input.
                        WindowEvent::KeyboardInput { input: key_event, .. } => match self.stage {
                            Stage::Running {
                                ref mut app_menu,
                                ref mut show_settings,
                                ..
                            } => {
                                match (key_event.virtual_keycode, key_event.state) {
                                    (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                                        app_menu.toggle_open(); 
                                    }
                                    (Some(VirtualKeyCode::Tab), ElementState::Pressed) => {
                                        *show_settings = !*show_settings; 
                                    }
                                    (Some(key), state) if *show_settings => {
                                        if !self.config.handle_key_open(key, state) {
                                            self.gui_ctx.handle_window_event(event);
                                        }
                                    }
                                    (Some(key), state) if !*show_settings => {
                                        if !self.config.handle_key_closed(key, state) {
                                            self.gui_ctx.handle_window_event(event);
                                        }
                                    }
                                    _ => {
                                        self.gui_ctx.handle_window_event(event);
                                    }
                                }
                            }
                            Stage::StartMenu(_) => {
                                if let Some(key) = key_event.virtual_keycode {
                                    self.config.handle_key_open(key, key_event.state);
                                }
                            }
                        }
                        _ => {
                            self.gui_ctx.handle_window_event(event);
                        }
                    }
                }
                Event::RedrawRequested(_) => match self.stage {
                    Stage::Running {
                        ref mut app_menu,
                        ref mut mode,
                        ref system,
                        show_settings,
                        ..
                    } => {
                        self.renderer.render(|encoder, view, renderer| {
                            let res = self.gui_ctx.render(renderer, encoder, view, &self.window, |gui| {
                                app_menu.show(gui, mode);
                                if show_settings {
                                    self.config.show(Some(system.bios()), gui);
                                }
                            });
                            if let Err(err) = res {
                                app_menu.close_apps();
                                error!("Failed to render GUI: {}", err);
                            }
                        });
                    }
                    Stage::StartMenu(ref mut menu) => {
                        let mut bios = None;

                        self.renderer.render(|encoder, view, renderer| {
                            let res = self.gui_ctx.render(renderer, encoder, view, &self.window, |gui| {
                                bios = menu.show_area(&mut self.config, gui);
                            });
                            if let Err(err) = res {
                                error!("Failed to render GUI: {}", err);
                            }
                        });

                        if let Some(bios) = bios {
                            self.stage = Stage::Running {
                                system: System::new(
                                    bios,
                                    self.config.disc.disc(),
                                    self.config.controller.controllers(),
                                ),
                                app_menu: Box::new(DebugMenu::new()),
                                last_update: Instant::now(),
                                mode: RunMode::Emulation,
                                show_settings: false,
                            }
                        }
                    }
                },
                Event::MainEventsCleared => match self.stage {
                    Stage::Running {
                        ref mut system,
                        ref mut app_menu,
                        ref mut last_update,
                        mode,
                        ..
                    } => {
                        match mode {
                            RunMode::Debug => {
                                app_menu.update_tick(
                                    last_update.elapsed(),
                                    system,
                                    &mut self.renderer
                                );
                            }
                            RunMode::Emulation => {
                                system.run(
                                    last_update.elapsed(),
                                    &mut self.renderer
                                );
                            }
                        }
                        *last_update = Instant::now();
                    },
                    Stage::StartMenu(..) => (),
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

        let renderer = Renderer::new(&window);
        let config = Config::load_from_file().unwrap_or_default();

        // TODO: Change frame rate to handle both PAL and NTSC.
        let frame_time = Duration::from_secs_f32(1.0 / timing::NTSC_FPS as f32);

        Self {
            stage: Stage::StartMenu(StartMenu::new()),
            gui_ctx: GuiCtx::new(window.scale_factor() as f32, &renderer),
            last_draw: Instant::now(),
            config,
            renderer,
            event_loop,
            window,
            frame_time,
        }
    }
}
