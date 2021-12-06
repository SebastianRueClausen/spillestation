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
    Canvas,
    DrawStage,
    ComputeStage,
};
use gui::{GuiCtx, app_menu::AppMenu};
use crate::cpu::Cpu;
use std::time::{Instant, Duration};

pub use render::compute::DrawInfo;

mod render;
pub mod gui;

pub fn run() {
    env_logger::init();
    let mut cpu = Cpu::new();
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
    let mut app_menu = AppMenu::new();
    let canvas = Canvas::new(&render_ctx.device, SurfaceSize {
        width: 640,
        height: 480,
    });
    let mut compute = ComputeStage::new(
        &render_ctx.device,
        &canvas
    );
    let mut draw = DrawStage::new(
        &render_ctx,
        &canvas,
    );
    let mut last_draw = Instant::now();
    let mut last_update = Instant::now();
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
                    app_menu.draw_tick(dt);
                    last_draw = Instant::now();
                } else {
                    *control_flow = ControlFlow::WaitUntil(
                        Instant::now() + Duration::from_secs_f32(1.0 / 60.0) - dt
                    );
                }
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
                        *control_flow = ControlFlow::Exit;
                    },
                    WindowEvent::Resized(physical_size) => {
                        let size = SurfaceSize::new(
                            physical_size.width,
                            physical_size.height,
                        );
                        render_ctx.resize(size);
                        draw.resize(&render_ctx, &canvas);
                        gui.resize(size);
                    },
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        render_ctx.resize(SurfaceSize::new(new_inner_size.width, new_inner_size.height));
                        draw.resize(&render_ctx, &canvas);
                        gui.set_scale_factor(window.scale_factor() as f32);
                    },
                    _ => {
                        gui.handle_window_event(event); 
                    },
                }
            },
            Event::RedrawRequested(_) => {
                render_ctx.render(|encoder, view, renderer| {
                    compute.compute_render_texture(
                        cpu.bus().vram(),
                        &cpu.bus().gpu().draw_info(),
                        encoder,
                        &renderer.queue,
                        &canvas,
                    );
                    draw.render(
                        encoder,
                        &view,
                    );
                    gui.begin_frame(&window);
                    app_menu.show_apps(&mut gui);
                    app_menu.show(&mut gui.egui_ctx);
                    gui.end_frame(&window);
                    gui.render(
                        renderer,
                        encoder,
                        view,
                    )
                    .expect("Failed to render gui");
                });
            },
            Event::MainEventsCleared => {
                app_menu.update_tick(last_update.elapsed(), &mut cpu);
                last_update = Instant::now(); 
            },
            _ => {
            },
        }
    });
}
