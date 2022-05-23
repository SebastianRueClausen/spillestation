#![feature(let_else)]

//! The frontend of the emulator. Handles the window, input/output, rendering, GUI and controls the
//! emulator itself.

#[macro_use]
extern crate log;

mod audio_stream;
mod config;
mod debug;
mod gui;
mod keys;
mod start_menu;

use audio_stream::AudioStream;
use config::Config;
use debug::DebugMenu;
use gui::GuiRenderer;
use splst_core::{io_port::pad, Disc, System};
use splst_render::{Renderer, SurfaceSize};

use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use std::cell::RefCell;
use std::rc::Rc;
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
    /// The start menu shown when starting the emulator.
    StartMenu,
    Running {
        system: System,
        app_menu: Box<DebugMenu>,
        /// The last time 'system' ran.
        last_update: Instant,
        mode: RunMode,
        show_settings: bool,
    },
}

pub fn run() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("spillestation")
        .build(&event_loop)
        .expect("failed to create window");

    let renderer = Rc::new(RefCell::new(Renderer::new(&window)));
    let controllers = Rc::new(RefCell::new(pad::Controllers::default()));
    let disc = Rc::new(RefCell::new(Disc::default()));

    // TODO: Show and error in the settings menu, but still allow the emulator to run without audio.
    let audio_stream = AudioStream::new().unwrap();
    let audio_stream = Rc::new(RefCell::new(audio_stream));

    let mut gui_renderer = GuiRenderer::new(window.scale_factor() as f32, &renderer.borrow());
    let mut stage = Stage::StartMenu;

    let mut config = Config::from_file_or_default(&mut gui_renderer.ctx());
    let mut key_map = config.controller.get_key_map(&mut gui_renderer.ctx());

    config
        .controller
        .update_controllers(&mut controllers.borrow_mut());

    // The instant the last frame was drawn.
    let mut last_draw = Instant::now();

    // The amonut of time between each frame. Only used in debug mode.
    let frame_time = Duration::from_secs_f32(1.0 / 60.0);

    event_loop.run(move |event, _, ctrl_flow| {
        *ctrl_flow = ControlFlow::Poll;
        match event {
            Event::RedrawEventsCleared => {
                let dt = last_draw.elapsed();

                let mut redraw = || {
                    window.request_redraw();
                    last_draw = Instant::now();
                };

                match stage {
                    Stage::Running {
                        ref mut app_menu,
                        mode,
                        ..
                    } => match mode {
                        RunMode::Emulation => {
                            if renderer.borrow().has_pending_frame() {
                                redraw();
                            }
                        }
                        RunMode::Debug => {
                            if dt >= frame_time || renderer.borrow().has_pending_frame() {
                                app_menu.draw_tick(dt);
                                redraw();
                            }
                        }
                    },
                    Stage::StartMenu { .. } => {
                        if dt >= frame_time {
                            redraw();
                        }
                    }
                }
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => *ctrl_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => {
                    let size = SurfaceSize {
                        width: physical_size.width,
                        height: physical_size.height,
                    };
                    renderer.borrow_mut().resize(size);
                    gui_renderer.resize(size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    renderer.borrow_mut().resize(SurfaceSize {
                        width: new_inner_size.width,
                        height: new_inner_size.height,
                    });
                    gui_renderer.set_scale_factor(window.scale_factor() as f32);
                }
                WindowEvent::DroppedFile(path) => match stage {
                    Stage::Running { show_settings, .. } => {
                        if show_settings {
                            config.handle_dropped_file(&path.as_path());
                        }
                    }
                    Stage::StartMenu => {
                        config.handle_dropped_file(&path.as_path());
                    }
                },
                WindowEvent::KeyboardInput {
                    input: key_event, ..
                } => match stage {
                    Stage::Running {
                        ref mut app_menu,
                        ref mut show_settings,
                        ..
                    } => match (key_event.virtual_keycode, key_event.state) {
                        (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                            app_menu.toggle_open();
                        }
                        (Some(VirtualKeyCode::Tab), ElementState::Pressed) => {
                            *show_settings = !*show_settings;
                        }
                        (Some(key), state) if *show_settings => {
                            if !config.handle_key_event(
                                &mut key_map,
                                key,
                                state,
                                gui_renderer.ctx(),
                            ) {
                                gui_renderer.handle_window_event(event);
                            }
                        }
                        (Some(key), state) if !*show_settings => {
                            let consumed = key_map
                                .get(&key)
                                .map(|(slot, button)| {
                                    controllers.borrow_mut()[*slot]
                                        .set_button(*button, state == ElementState::Pressed);
                                })
                                .is_some();

                            if !consumed {
                                gui_renderer.handle_window_event(event);
                            }
                        }
                        _ => {
                            gui_renderer.handle_window_event(event);
                        }
                    },
                    Stage::StartMenu { .. } => {
                        if let Some(key) = key_event.virtual_keycode {
                            if !config.handle_key_event(
                                &mut key_map,
                                key,
                                key_event.state,
                                gui_renderer.ctx(),
                            ) {
                                gui_renderer.handle_window_event(event);
                            }
                        }
                    }
                },
                _ => {
                    gui_renderer.handle_window_event(event);
                }
            },
            Event::RedrawRequested(_) => match stage {
                Stage::Running {
                    ref mut app_menu,
                    ref mut mode,
                    ref mut system,
                    show_settings,
                    ..
                } => {
                    renderer.borrow_mut().render(|encoder, view, renderer| {
                        let res = gui_renderer.render(renderer, encoder, view, &window, |ctx| {
                            app_menu.show(ctx, system, mode);

                            if show_settings {
                                config.show(
                                    Some(system.bios()),
                                    &mut controllers.borrow_mut(),
                                    &mut disc.borrow_mut(),
                                    ctx,
                                );
                            }
                        });
                        if let Err(err) = res {
                            app_menu.close_apps();
                            error!("Failed to render GUI: {}", err);
                        }
                    });
                }
                Stage::StartMenu => {
                    let mut bios = None;
                    renderer.borrow_mut().render(|encoder, view, renderer| {
                        let res = gui_renderer.render(renderer, encoder, view, &window, |ctx| {
                            bios = start_menu::show(
                                &mut config,
                                &mut controllers.borrow_mut(),
                                &mut disc.borrow_mut(),
                                ctx,
                            );
                        });
                        if let Err(err) = res {
                            error!("Failed to render GUI: {}", err);
                        }
                    });

                    if let Some(bios) = bios {
                        let mut system = System::new(
                            bios,
                            renderer.clone(),
                            audio_stream.clone(),
                            disc.clone(),
                            controllers.clone(),
                        );

                        if let Some(exe) = config.exe.take_exe() {
                            system.load_exe(&exe);
                        }

                        stage = Stage::Running {
                            system,
                            app_menu: Box::new(DebugMenu::default()),
                            last_update: Instant::now(),
                            mode: RunMode::Emulation,
                            show_settings: false,
                        }
                    }
                }
            },
            Event::MainEventsCleared => match stage {
                Stage::Running {
                    ref mut system,
                    ref mut app_menu,
                    ref mut last_update,
                    mode,
                    ..
                } => {
                    match mode {
                        RunMode::Debug => app_menu.update_tick(last_update.elapsed(), system),
                        RunMode::Emulation => {
                            let run_time = Duration::from_millis(1);
                            let before = Instant::now();

                            system.run(run_time);

                            if let Some(ahead) = run_time.checked_sub(before.elapsed()) {
                                if ahead > Duration::from_micros(10) {
                                    trace!("sleeping for {} micros", ahead.as_millis());
                                    *ctrl_flow = ControlFlow::WaitUntil(Instant::now() + ahead);
                                }
                            }
                        }
                    }
                    *last_update = Instant::now();
                }
                Stage::StartMenu { .. } => (),
            },
            _ => {}
        }
    });
}
