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

mod renderer;

pub struct Frontend {
}

impl Frontend {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(&mut self) {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Spillestation")
            .build(&event_loop)
            .unwrap();
        event_loop.run(move |event, _, control_flow| {
            // This means the event loop is runs all the time, even if there isn't any events.
            *control_flow = ControlFlow::Poll;
            match event {
                Event::MainEventsCleared => {
                    window.request_redraw();
                },
                Event::DeviceEvent { ref event, .. } => {
                    // TODO: Handle input.
                },
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
                            // TODO: Handle resize.
                        },
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            // TODO: Handle resize.
                        },
                        _ => {},
                    }
                },
                Event::RedrawRequested(_) => {
                    
                },
                _ => {
                },
            }
        });
    }
}
