//! The frontend of the emulator. Handles the window, input/output, rendering, GUI and controls the
//! emulator itself.

mod config;
pub mod gui;
mod render;

use crate::{system::System, memory::bios::Bios, timing};
use config::Config;
use gui::{App, app_menu::AppMenu, GuiCtx, config::Configurator};
use render::{Canvas, ComputeStage, DrawStage, RenderCtx, SurfaceSize};
use std::{path::Path, time::{Duration, Instant}};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
pub use render::compute::DrawInfo;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Debug,
    Emulation,
}

enum State {
    Startup,
    Config {
        configurator: Configurator,
        bios: Option<Bios>,
    },
    Running {
        system: System,
        app_menu: Box<AppMenu>,
        last_update: Instant,
        mode: RunMode,
    },
}

/// The main loop running the emulator.
pub fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("spillestation")
        .build(&event_loop)
        .expect("Failed to create window");
    let mut render_ctx = RenderCtx::new(&window);
    let mut gui = GuiCtx::new(window.scale_factor() as f32, &render_ctx);
    let canvas = Canvas::new(&render_ctx.device, SurfaceSize::new(640, 480));
    let mut compute = ComputeStage::new(&render_ctx.device, &canvas);
    let mut draw = DrawStage::new(&render_ctx, &canvas);
    let mut last_draw = Instant::now();
    let mut state = State::Startup;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::RedrawEventsCleared => {
                let frame_time = Duration::from_secs_f32(1.0 / timing::PAL_FPS as f32);
                let dt = last_draw.elapsed();
                if dt >= frame_time {
                    window.request_redraw();
                    if let State::Running {
                        ref mut app_menu,
                        ref mut system,
                        mode,
                        ..
                    } = state {
                        match mode {
                            RunMode::Debug => app_menu.draw_tick(dt),
                            RunMode::Emulation => system.cpu.bus_mut().irq_state.trigger(crate::cpu::irq::Irq::VBlank),
                        }
                    }
                    last_draw = Instant::now();
                } else {
                    *control_flow = ControlFlow::WaitUntil(
                        Instant::now() + frame_time - dt,
                    );
                }
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(physical_size) => {
                        let size = SurfaceSize::new(physical_size.width, physical_size.height);
                        render_ctx.resize(size);
                        draw.resize(&render_ctx, &canvas);
                        gui.resize(size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        render_ctx.resize(SurfaceSize::new(
                            new_inner_size.width,
                            new_inner_size.height,
                        ));
                        draw.resize(&render_ctx, &canvas);
                        gui.set_scale_factor(window.scale_factor() as f32);
                    }
                    // Handle keyboard input.
                    WindowEvent::KeyboardInput { input, .. } => {
                        if let State::Running { ref mut app_menu, .. } = state {
                            match (input.virtual_keycode, input.state) {
                                (Some(VirtualKeyCode::M), ElementState::Pressed) => {
                                    app_menu.toggle_open();
                                }
                                (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                                    *control_flow = ControlFlow::Exit;
                                }
                                _ => {}
                            }
                        }
                        gui.handle_window_event(event);
                    }
                    _ => {
                        gui.handle_window_event(event);
                    }
                }
            }
            Event::RedrawRequested(_) => match state {
                State::Running {
                    ref mut system,
                    ref mut app_menu,
                    ref mut mode,
                    ..
                } => {
                    render_ctx.render(|encoder, view, renderer| {
                        compute.compute_render_texture(
                            system.cpu.bus().vram(),
                            &system.cpu.bus().gpu().draw_info(),
                            encoder,
                            &renderer.queue,
                            &canvas,
                        );
                        draw.render(encoder, view);
                        let res = gui.render(renderer, encoder, view, &window, |gui| {
                            app_menu.show(gui, mode);
                        });
                        if let Err(ref err) = res {
                            app_menu.close_apps();
                            eprintln!("{}", err);
                        }
                    });
                }
                State::Config { ref mut configurator, ref mut bios } => {
                    let mut config_open = false;
                    render_ctx.render(|encoder, view, renderer| {
                        draw.render(encoder, view);
                        let res = gui.render(renderer, encoder, view, &window, |gui| {
                            configurator.show_window(gui, &mut config_open);
                        });
                        if let Err(ref err) = res {
                            eprintln!("{}", err);
                        }
                    });
                    if !config_open {
                        match bios.take() {
                            Some(bios) => {
                                if let Err(err) = configurator.config.store() {
                                    eprintln!("{}", err)
                                }
                                state = State::Running {
                                    system: System::new(bios),
                                    app_menu: Box::new(AppMenu::new()),
                                    last_update: Instant::now(),
                                    mode: RunMode::Emulation,
                                };
                            },
                            None => {
                                configurator.bios_message = Some(
                                    String::from("A BIOS file must be loaded")
                                );
                            },
                        }
                    }
                }
                State::Startup => {}
            },
            Event::MainEventsCleared => match state {
                State::Running {
                    ref mut system,
                    ref mut app_menu,
                    ref mut last_update,
                    mode,
                } => {
                    match mode {
                        RunMode::Debug => {
                            app_menu.update_tick(last_update.elapsed(), system);
                        }
                        RunMode::Emulation => system.run(last_update.elapsed()),
                    }
                    *last_update = Instant::now();
                },
                State::Config {
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
                State::Startup => {
                    state = match Config::load() {
                        Ok(config) => match Bios::from_file(Path::new(&config.bios)) {
                            Err(ref err) => State::Config {
                                configurator: Configurator::new(
                                    None,
                                    Some(format!("{}", err)),
                                ),
                                bios: None,
                            },
                            Ok(bios) => State::Running {
                                system: System::new(bios),
                                app_menu: Box::new(AppMenu::new()),
                                last_update: Instant::now(),
                                mode: RunMode::Emulation,
                            },
                        },
                        Err(ref err) => State::Config {
                            configurator: Configurator::new(
                                Some(format!("{}", err)),
                                None,
                            ),
                            bios: None,
                        },
                    };
                }
                _ => {},
            },
            _ => {},
        }
    });
}
