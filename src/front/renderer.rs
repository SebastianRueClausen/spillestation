use crate::gpu::vram::{Vram, VRAM_SIZE};
use ultraviolet::{Mat4, Vec2};
use wgpu::util::DeviceExt;
use winit::window::Window;

struct ComputePipeline {
    input_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl ComputePipeline {
    fn new(device: &wgpu::Device, render_texture: &RenderTexture) -> Self {
        let shader = device.create_shader_module(&wgpu::include_spirv!("shader/comp.spv"));
        let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Compute Storage Buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
            size: VRAM_SIZE as u64,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: RENDER_TEXTURE_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&render_texture.view),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Shader Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        Self {
            input_buffer,
            bind_group,
            pipeline,
        }
    }

    fn compute_render_texture(
        &mut self,
        vram: &Vram,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        render_texture: &RenderTexture,
    ) {
        queue.write_buffer(&self.input_buffer, 0, vram.raw_data());
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch(render_texture.extent.width, render_texture.extent.height, 1);
    }
}

#[derive(Clone, Copy, Debug)]
struct ScissorRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

struct RenderPipeline {
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    /*
    width: f32,
    height: f32,
    */
    scissor_rect: ScissorRect,
}

impl RenderPipeline {
    fn new(
        device: &wgpu::Device,
        surface_size: SurfaceSize,
        surface_format: wgpu::TextureFormat,
        render_texture: &RenderTexture,
    ) -> Self {
        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: None,
            border_color: None,
        });
        let vertex_data = bytemuck::cast_slice(&VERTICES);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: vertex_data,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: (vertex_data.len() / 3) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position.
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // TexCoord.
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        };
        let (transform, scissor_rect) = generate_transform_matrix(
            Vec2::new(
                render_texture.extent.width as f32,
                render_texture.extent.height as f32,
            ),
            Vec2::new(surface_size.width as f32, surface_size.height as f32),
        );
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transform Buffer"),
            contents: transform.as_byte_slice(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: true,
                        },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler {
                        filtering: true,
                        comparison: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&render_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });
        let vert = device.create_shader_module(&wgpu::include_spirv!("shader/vert.spv"));
        let frag = device.create_shader_module(&wgpu::include_spirv!("shader/frag.spv"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vert,
                entry_point: "main",
                buffers: &[vertex_buffer_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &frag,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        });
        Self {
            scissor_rect,
            bind_group,
            uniform_buffer,
            vertex_buffer,
            /*
            width: render_texture.extent.width as f32,
            height: render_texture.extent.height as f32,
            */
            pipeline,
        }
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        println!("{:?}", self.scissor_rect);
        render_pass.set_scissor_rect(
            self.scissor_rect.x,
            self.scissor_rect.y,
            self.scissor_rect.width,
            self.scissor_rect.height,
        );
        render_pass.draw(0..3, 0..1);
    }

    fn resize(
        &mut self,
        queue: &wgpu::Queue,
        surface_size: SurfaceSize,
        render_texture: &RenderTexture,
    ) {
        let (transform, scissor_rect) = generate_transform_matrix(
            Vec2::new(
                render_texture.extent.width as f32,
                render_texture.extent.height as f32,
            ),
            Vec2::new(surface_size.width as f32, surface_size.height as f32),
        );
        self.scissor_rect = scissor_rect;
        queue.write_buffer(&self.uniform_buffer, 0, transform.as_byte_slice());
    }
}

struct RenderTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: wgpu::Extent3d,
}

const RENDER_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

impl RenderTexture {
    fn new(device: &wgpu::Device, surface_size: (u32, u32)) -> Self {
        let (width, height) = surface_size;
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::STORAGE_BINDING,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            extent,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SurfaceSize {
    width: u32,
    height: u32,
}

impl SurfaceSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    surface_size: SurfaceSize,
    render_texture: RenderTexture,
    render_pipeline: RenderPipeline,
    compute_pipeline: ComputePipeline,
}

impl Renderer {
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
                present_mode: wgpu::PresentMode::Fifo,
            },
        );
        let render_texture = RenderTexture::new(&device, (width, height));
        let render_pipeline =
            RenderPipeline::new(&device, surface_size, surface_format, &render_texture);
        let compute_pipeline = ComputePipeline::new(&device, &render_texture);
        Self {
            device,
            queue,
            surface,
            surface_format,
            surface_size,
            render_texture,
            render_pipeline,
            compute_pipeline,
        }
    }

    pub fn render(&mut self, vram: &Vram) {
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
        self.compute_pipeline.compute_render_texture(
            vram,
            &mut encoder,
            &self.queue,
            &self.render_texture,
        );
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.render_pipeline.render(&mut encoder, &view);
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
            self.render_pipeline
                .resize(&self.queue, surface_size, &self.render_texture);
        }
    }
}

fn generate_transform_matrix(texture: Vec2, screen: Vec2) -> (Mat4, ScissorRect) {
    let scale = texture
        * (screen.x / texture.x)
            .min(screen.y / texture.y)
            .max(1.0)
            .floor();
    let scaled = scale * screen;
    let scaled_screen = scaled / screen;
    let scaled_texture = Vec2::new(
        (texture.x / screen.x - 1.0).max(0.0),
        (1.0 - texture.y / screen.y).min(0.0),
    );
    let translation = Mat4::from([
        [scaled_screen.x, 0.0, 0.0, 0.0],
        [0.0, -scaled_screen.y, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [scaled_texture.x, scaled_texture.y, 0.0, 1.0],
    ]);
    let scaled = Vec2::new(
        scaled.x.min(screen.x),
        scaled.y.min(screen.y),
    );
    let clip = (screen - scaled) * 0.5;
    let scissor_rect = ScissorRect {
        x: clip.x as u32,
        y: clip.y as u32,
        width: scaled.x as u32,
        height: scaled.y as u32,
    };
    (translation, scissor_rect)
}

/// One large triangle, which get's clipped to [0, 0] and [1, 1].
const VERTICES: [[[f32; 2]; 2]; 3] = [
    [[-1.0, -1.0], [0.0, 0.0]],
    [[3.0, -1.0], [2.0, 0.0]],
    [[-1.0, 3.0], [0.0, 2.0]],
];
