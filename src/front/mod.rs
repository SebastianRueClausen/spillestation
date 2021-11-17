use winit::{
    window::WindowBuilder,
    event_loop::{
        EventLoop, ControlFlow,
    },
    event::{
        Event,
        WindowEvent,
        KeyboardInput,
        ElementState,
        VirtualKeyCode,
    },
};
use renderer::{
    RenderCtx,
    SurfaceSize,
};
use crate::cpu::Cpu;

mod renderer;
mod gui;

pub fn run() {
    env_logger::init();
    let mut cpu = Cpu::new();
    cpu.fetch_and_exec();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Spillestation")
        .build(&event_loop)
        .unwrap();
    let mut renderer = RenderCtx::new(&window);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared => {
                window.request_redraw();
            },
            /*
            Event::DeviceEvent { ref event, .. } => {
                // TODO: Handle input.
            },
            */
            Event::WindowEvent {
                ref event, window_id,
            } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => {
                        // Close window.
                        *control_flow = ControlFlow::Exit;
                    },
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(SurfaceSize::new(physical_size.width, physical_size.height));
                    },
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        renderer.resize(SurfaceSize::new(new_inner_size.width, new_inner_size.height));
                        renderer.gui.set_scale_factor(window.scale_factor() as f32);
                    },
                    _ => {
                        renderer.gui.handle_window_event(event); 
                    },
                }
            },
            Event::RedrawRequested(_) => {
                renderer.render(&window, cpu.bus().vram());
            },
            _ => {
            },
        }
    });
}
