use winit::window::Window;

pub use compute::ComputeStage;
pub use draw::DrawStage;

pub mod compute;
pub mod draw;


/// Render Texture. This is the texture drawn to the screen each frame. This is generated by
/// ['ComputePipeline'] and drawn by ['RenderPipeline']. 
pub struct RenderTexture {
    view: wgpu::TextureView,
    extent: wgpu::Extent3d,
}

/// The format of ['RenderTexture']. Signed and unsigned int formats was a pain, so the reason it's
/// 16 bit floating points is simply because that's what seemed to work.
pub const RENDER_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

impl RenderTexture {
    pub fn new(device: &wgpu::Device, surface_size: SurfaceSize) -> Self {
        let SurfaceSize { width, height } = surface_size;
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: RENDER_TEXTURE_FORMAT,
            // I'm a bit unsure which usage flags would be optimal. Maybe COPY_DST, but it's not
            // really copied to but written to pixel by pixel by the compute shader. But it doesn't
            // seem to change performance really, so perhaps it doesn't matter.
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            view,
            extent,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl SurfaceSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Render Context. Simply holds all the wgpu stuff needed to render an image to the screen.
pub struct RenderCtx {
    /// The device used to render. This is picked by wgpu, as we don't really need any non-standard
    /// features or anything like that.
    pub device: wgpu::Device,
    /// Command queue. Required to render stuff.
    pub queue: wgpu::Queue,
    /// Format of the surface texture. This is just picked by wgpu, which pick's the preferred by
    /// the hardward. 
    pub surface_format: wgpu::TextureFormat,
    pub surface_size: SurfaceSize,
    surface: wgpu::Surface,
}

impl RenderCtx {
    pub fn new(window: &Window) -> Self {
        let (width, height) = (window.inner_size().width, window.inner_size().height);
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        ))
        .unwrap();
        let surface_format = surface.get_preferred_format(&adapter).unwrap();
        let surface_size = SurfaceSize { width, height };
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width,
                height,
                present_mode: wgpu::PresentMode::Mailbox,
            },
        );
        Self {
            device,
            queue,
            surface,
            surface_format,
            surface_size,
        }
    }

    /// The main function used to render stuff. It prepares everything from wgpu needed to start
    /// rendering, runs the render function, and finally it submit's the commands and present's the
    /// rendered frame.
    pub fn render<F>(&mut self, func: F)
    where
        F: FnOnce(
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            &Self,
        ),
    {
        let frame = self
            .surface
            .get_current_texture()
            .or_else(|err| match err {
                wgpu::SurfaceError::Outdated => {
                    self.configure_surface();
                    self.surface.get_current_texture()
                }
                _ => panic!("Surface Error {}", err),
            })
            .unwrap();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("") });
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        (func)(&mut encoder, &view, self);
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn configure_surface(&mut self) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface_format,
                width: self.surface_size.width,
                height: self.surface_size.height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        );
    }

    pub fn resize(&mut self, surface_size: SurfaceSize) {
        if surface_size.width != 0 && surface_size.height != 0 {
            self.surface_size = surface_size;
            self.configure_surface();
        }
    }
}
