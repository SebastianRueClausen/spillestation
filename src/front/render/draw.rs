use super::{Canvas, RenderCtx};
use ultraviolet::{Mat4, Vec2};
use wgpu::util::DeviceExt;

/// This is the Scissor Rectangle used by ['DrawStage']. It's used after the fragment shader
/// stage as far as i'm avare, and it determines the viewable area of the viewport.
#[derive(Clone, Copy, Debug)]
struct ScissorRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// The draw stage for displaying the ['Canvas'] on screen. It's for the most part
/// very simple. It draws a single triangle, which get's textured by the ['Canvas']. The
/// only complicated part is the transforming and clipping of the texture to display it correctly.
pub struct DrawStage {
    /// Vertex buffex. It only contains a sinlge trianlge. Each vertex has a position and texture coordinate and it's
    /// never updated after creation.
    vertex_buffer: wgpu::Buffer,
    /// The uniform buffer contains the transformation matrix generated by
    /// ['generate_transform_matrix'].
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    scissor_rect: ScissorRect,
}

impl DrawStage {
    pub fn new(ctx: &RenderCtx, canvas: &Canvas) -> Self {
        let texture_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
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
        let vertex_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: vertex_data,
                usage: wgpu::BufferUsages::VERTEX,
            });
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: (vertex_data.len() / 3) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        };
        let (transform, scissor_rect) = generate_transform_matrix(
            Vec2::new(canvas.extent.width as f32, canvas.extent.height as f32),
            Vec2::new(
                ctx.surface_size.width as f32,
                ctx.surface_size.height as f32,
            ),
        );
        let uniform_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Transform Buffer"),
                contents: transform.as_byte_slice(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Render Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&canvas.view),
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

        let vert = ctx
            .device
            .create_shader_module(&wgpu::include_spirv!("shader/vert.spv"));
        let frag = ctx
            .device
            .create_shader_module(&wgpu::include_spirv!("shader/frag.spv"));

        let pipeline_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
        let pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                        format: ctx.surface_format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::REPLACE,
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multiview: None,
                multisample: wgpu::MultisampleState::default(),
            });
        Self {
            scissor_rect,
            bind_group,
            uniform_buffer,
            vertex_buffer,
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
        render_pass.set_scissor_rect(
            self.scissor_rect.x,
            self.scissor_rect.y,
            self.scissor_rect.width,
            self.scissor_rect.height,
        );
        // Draw one instance of the triangle ['VERTICES'].
        render_pass.draw(0..3, 0..1);
    }

    pub fn resize(&mut self, ctx: &RenderCtx, canvas: &Canvas) {
        let (transform, scissor_rect) = generate_transform_matrix(
            Vec2::new(canvas.extent.width as f32, canvas.extent.height as f32),
            Vec2::new(
                ctx.surface_size.width as f32,
                ctx.surface_size.height as f32,
            ),
        );
        self.scissor_rect = scissor_rect;
        ctx.queue
            .write_buffer(&self.uniform_buffer, 0, transform.as_byte_slice());
    }
}

/// Generates the transform matrix used by the fragment shader, and the scissor rectangle used by
/// the render pipeline. It depends on size of the surface texture and the render texture, so it
/// must be recaluculated each time on of these change.
fn generate_transform_matrix(texture: Vec2, screen: Vec2) -> (Mat4, ScissorRect) {
    // The smallest scale ratio.
    let scale = (screen.x / texture.x)
        .min(screen.y / texture.y)
        .max(1.0)
        .floor();
    // Scaled tetxure dimension.
    let scaled = texture * scale;
    // Scaling of the vertices.
    let s = scaled / screen;
    // Translation of the vertices. The min/max just makes sure the texture doesn't go off screen.
    let t = Vec2::new(
        (texture.x / screen.x - 1.0).max(0.0),
        (1.0 - texture.y / screen.y).min(0.0),
    );
    // Transformation matrix. It flips the image vertically since the Playstations coordinates in VRAM
    // are the opposite of wgpu.
    let transform = Mat4::from([
        [s.x, 0.0, 0.0, 0.0],
        [0.0, -s.y, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [t.x, t.y, 0.0, 1.0],
    ]);
    // The clipping rectangle width and height.
    let clip_wh = Vec2::new(scaled.x.min(screen.x), scaled.y.min(screen.y));
    // The clipping rectangle upper right corner.
    let clip_xy = (screen - clip_wh) * 0.5;
    let scissor_rect = ScissorRect {
        x: clip_xy.x as u32,
        y: clip_xy.y as u32,
        width: clip_wh.x as u32,
        height: clip_wh.y as u32,
    };
    (transform, scissor_rect)
}

/// One large triangle, which get's clipped to (0, 0) and (1, 1).
const VERTICES: [[[f32; 2]; 2]; 3] = [
    [[-1.0, -1.0], [0.0, 0.0]],
    [[3.0, -1.0], [2.0, 0.0]],
    [[-1.0, 3.0], [0.0, 2.0]],
];
