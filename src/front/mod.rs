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
use render::{
    RenderCtx,
    SurfaceSize,
    RenderTexture,
    DrawStage,
    ComputeStage,
};
use gui::{
    GuiCtx,
    fps::FrameCounter,
};
use crate::cpu::Cpu;

mod render;
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

    let mut render_ctx = RenderCtx::new(&window);

    let mut gui = GuiCtx::new(
        window.scale_factor() as f32,
        &render_ctx,
    );

    let mut fps = FrameCounter::new();

    let render_texture = RenderTexture::new(&render_ctx.device, SurfaceSize {
        width: 640,
        height: 480,
    });

    let mut compute = ComputeStage::new(
        &render_ctx.device,
        &render_texture

    );
    let mut draw = DrawStage::new(
        &render_ctx,
        &render_texture,
    );

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
                        let size = SurfaceSize::new(
                            physical_size.width,
                            physical_size.height,
                        );
                        render_ctx.resize(size);
                        draw.resize(&render_ctx, &render_texture);
                        gui.resize(size);
                    },
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        render_ctx.resize(SurfaceSize::new(new_inner_size.width, new_inner_size.height));
                        draw.resize(&render_ctx, &render_texture);
                        gui.set_scale_factor(window.scale_factor() as f32);
                    },
                    _ => {
                        gui.handle_window_event(event); 
                    },
                }
            },
            Event::RedrawRequested(_) => {
                render_ctx.render(|encoder, view, renderer| {
                    // Genrate render texture.
                    compute.compute_render_texture(
                        cpu.bus().vram(),
                        encoder,
                        &renderer.queue,
                        &render_texture,
                    );
                    draw.render(
                        encoder,
                        &view,
                    );
                    gui.begin_frame(&window);
                    gui.show_app(&mut fps); 
                    gui.end_frame(&window);
                    gui.render(
                        encoder,
                        view,
                        &renderer.device,
                        &renderer.queue,
                    ).expect("Failed to render gui");
                });
            },
            _ => {
            },
        }
    });
}
