//! The frontend of the emulator. Handles the window, input/output, rendering, GUI and controls the
//! emulator itself.

mod config;
pub mod gui;
mod render;

use crate::cpu::Cpu;
use crate::memory::bios::Bios;
use config::Config;
use gui::{app_menu::AppMenu, GuiCtx, config::Configurator};
pub use render::compute::DrawInfo;
use render::{Canvas, ComputeStage, DrawStage, RenderCtx, SurfaceSize};
use std::path::Path;
use std::time::{Duration, Instant};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

enum State {
    Startup,
    Config {
        configurator: Configurator,
        bios: Option<Bios>,
    },
    Running {
        cpu: Cpu,
        app_menu: AppMenu,
        last_update: Instant,
    },
}

/// The main loop running the emulator.
pub fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Spillestation")
        .build(&event_loop)
        .expect("Failed to create window");
    let mut render_ctx = RenderCtx::new(&window);
    let mut gui = GuiCtx::new(window.scale_factor() as f32, &render_ctx);
    let canvas = Canvas::new(
        &render_ctx.device,
        SurfaceSize {
            width: 640,
            height: 480,
        },
    );
    let mut compute = ComputeStage::new(&render_ctx.device, &canvas);
    let mut draw = DrawStage::new(&render_ctx, &canvas);
    let mut last_draw = Instant::now();
    let mut state = State::Startup;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::RedrawEventsCleared => {
                // Lock frame to 60 fps. This seems to be required to avoid huge memory leaks on
                // mac, whe you minimize or switch workspace. It also runs much smoother for some
                // reason.
                let dt = last_draw.elapsed();
                if dt >= Duration::from_secs_f32(1.0 / 60.0) {
                    window.request_redraw();
                    if let State::Running { ref mut app_menu, .. } = state {
                        app_menu.draw_tick(dt);
                    }
                    last_draw = Instant::now();
                } else {
                    *control_flow = ControlFlow::WaitUntil(
                        Instant::now() + Duration::from_secs_f32(1.0 / 60.0) - dt,
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
                        match state {
                            State::Running {
                                ref mut app_menu, ..
                            } => match (input.virtual_keycode, input.state) {
                                (Some(VirtualKeyCode::M), ElementState::Pressed) => {
                                    app_menu.open = !app_menu.open;
                                }
                                (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                                    *control_flow = ControlFlow::Exit;
                                }
                                _ => {}
                            },
                            _ => {}
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
                    ref mut cpu,
                    ref mut app_menu,
                    ..
                } => {
                    render_ctx.render(|encoder, view, renderer| {
                        compute.compute_render_texture(
                            cpu.bus().vram(),
                            &cpu.bus().gpu().draw_info(),
                            encoder,
                            &renderer.queue,
                            &canvas,
                        );
                        draw.render(encoder, view);
                        gui.begin_frame(&window);
                        app_menu.show_apps(&mut gui);
                        app_menu.show(&gui.egui_ctx);
                        gui.end_frame(&window);
                        if let Err(ref err) = gui.render(renderer, encoder, view) {
                            app_menu.close_apps();
                            eprintln!("{}", err);
                        }
                    });
                }
                State::Config { ref mut configurator, ref mut bios } => {
                    let mut try_close = false;
                    render_ctx.render(|encoder, view, renderer| {
                        draw.render(encoder, view);
                        gui.begin_frame(&window);
                        try_close = !gui.show_app(configurator);
                        gui.end_frame(&window);
                        gui.render(renderer, encoder, view).expect("Failed to render GUI");
                    });
                    if try_close {
                        match bios.take() {
                            Some(bios) => {
                                if let Err(err) = configurator.config.store() {
                                    eprintln!("{}", err)
                                }
                                state = State::Running {
                                    cpu: Cpu::new(bios),
                                    app_menu: AppMenu::new(),
                                    last_update: Instant::now(),
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
                    ref mut cpu,
                    ref mut app_menu,
                    ref mut last_update,
                } => {
                    app_menu.update_tick(last_update.elapsed(), cpu);
                    *last_update = Instant::now();
                },
                State::Config {
                    ref mut configurator,
                    ref mut bios,
                } if configurator.try_load_bios => {
                    match Bios::from_file(&Path::new(&configurator.config.bios)) {
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
                        Ok(config) => match Bios::from_file(&Path::new(&config.bios)) {
                            Err(ref err) => State::Config {
                                configurator: Configurator::new(
                                    None,
                                    Some(format!("{}", err)),
                                ),
                                bios: None,
                            },
                            Ok(bios) => State::Running {
                                cpu: Cpu::new(bios),
                                app_menu: AppMenu::new(),
                                last_update: Instant::now(),
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
