#![feature(vec_retain_mut)]

//! The frontend of the emulator. Handles the window, input/output, rendering, GUI and controls the
//! emulator itself.
//!
//! TODO:
//! - Find some way to make the emulator sleep in emulation mode if we are keeping up. Right now it
//!   will just continue to run for very small intervals all the time.

#[macro_use]
extern crate log;

mod config;
mod gui;
mod render;

use splst_core::{System, Bios, timing, Input, Button};
use crate::render::{Renderer, SurfaceSize};
use config::Config;
use gui::{app_menu::AppMenu, GuiCtx};
use gui::start_menu::StartMenu;

use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Window};

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::collections::HashMap;

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
        key_bindings: HashMap<VirtualKeyCode, Button>,
        input: Input,
        app_menu: Box<AppMenu>,
        /// The last time 'system' ran.
        last_update: Instant,
        mode: RunMode,
    },
}

/// The frontend of the emulator. It handles the window, input/output, config,
/// running the emulator and rendering the output of the playstation to.
pub struct Frontend {
    stage: Stage,
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
                        WindowEvent::KeyboardInput { input: key_event, .. } => {
                            if let Stage::Running {
                                ref mut app_menu,
                                ref mut input,
                                ref key_bindings,
                                ..
                            } = self.stage {
                                match (key_event.virtual_keycode, key_event.state) {
                                    (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                                        app_menu.toggle_open(); 
                                    }
                                    (Some(key), ElementState::Pressed) => {
                                        if let Some(button) = key_bindings.get(&key) {
                                            input.controllers[0].set_button(button, true);
                                        }
                                    }
                                    _ => {
                                        self.gui_ctx.handle_window_event(event);
                                    }
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
                        ..
                    } => {
                        self.renderer.render(|encoder, view, renderer| {
                            let res = self.gui_ctx.render(renderer, encoder, view, &self.window, |gui| {
                                app_menu.show(gui, mode);
                            });
                            if let Err(ref err) = res {
                                app_menu.close_apps();
                                error!("Failed to render GUI: {}", err);
                            }
                        });
                    }
                    Stage::StartMenu(ref mut menu) => {
                        let mut items = None;

                        self.renderer.render(|encoder, view, renderer| {
                            let res = self.gui_ctx.render(renderer, encoder, view, &self.window, |gui| {
                                items = menu.show_area(gui);
                            });
                            if let Err(ref err) = res {
                                error!("Failed to render GUI: {}", err);
                            }
                        });

                        if let Some((bios, cd, key_bindings)) = items {
                            self.stage = Stage::Running {
                                system: System::new(bios, cd),
                                app_menu: Box::new(AppMenu::new()),
                                last_update: Instant::now(),
                                mode: RunMode::Emulation,
                                key_bindings,
                                input: Input::new(),
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
                                app_menu.update_tick(last_update.elapsed(), system, &mut self.renderer);
                            }
                            RunMode::Emulation => {
                                system.run(last_update.elapsed(), &mut self.renderer);
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

        // TODO: Change frame rate to handle both PAL and NTSC.
        let frame_time = Duration::from_secs_f32(1.0 / timing::NTSC_FPS as f32);

        let start_menu = match Config::load() {
            Err(err) => {
                trace!("Failed to load/find config file");
                StartMenu::new(None, None, Some(err.to_string()))
            }
            Ok(config) => {
                match Bios::from_file(Path::new(&config.bios)) {
                    Err(err) => {
                        trace!("Failed to read BIOS from config file");
                        StartMenu::new(
                            None,
                            Some(config.key_bindings),
                            Some(err.to_string()),
                        )
                    }
                    Ok(bios) => {
                        StartMenu::new(
                            Some(WithPath::new(bios, config.bios.clone())),
                            Some(config.key_bindings),
                            None,
                        )
                    }
                }
            }
        };

        Self {
            stage: Stage::StartMenu(start_menu),
            gui_ctx: GuiCtx::new(window.scale_factor() as f32, &renderer),
            last_draw: Instant::now(),
            renderer,
            event_loop,
            window,
            frame_time,
        }
    }
}

pub struct WithPath<T> {
    item: T,
    name: String,
    path: PathBuf, 
}

impl<T> WithPath<T> {
    fn new(item: T, path: PathBuf) -> Self {
        let name = path.components()
            .last()
            .map(|c| c.as_os_str())
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string();

        Self { item, path, name }
    }
}
