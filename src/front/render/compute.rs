use crate::gpu::vram::{Vram,VRAM_SIZE};
use super::{RenderTexture, RENDER_TEXTURE_FORMAT};

/// Used to generate the ['RenderTexture'] from the playstation VRAM directly using compute shaders.
/// This is called before every rendered frame. This means that 1 mb of data is uploaded to the GPU
/// each frame, which could be quite expensive. However generating the texture on the CPU would
/// take a lot of time, and the generated texture, which would be almost as big or bigger, still has to
/// transfered to the GPU.
pub struct ComputeStage {
    /// The playstation VRAM is transfered to this buffer each frame. It's 1 mb big, so it's
    /// probably gioing to be a bottleneck on some systems.
    input_buffer: wgpu::Buffer,
    /// The compute shader has two bindings:
    /// '''ignore
    /// layout (set = 0, binding = 0) uniform Vram {
	  ///     uint vram[(1024 * 1024) / 4];
    /// };
    /// layout (set = 0, binding = 1, rgba16f) uniform writeonly image2D tex;
    /// '''
    /// The first is 'input_buffer', the second is ['RenderTexture'].
    bind_group: wgpu::BindGroup,
    /// The dispatches the compute shader for each pixel in ['RenderTexture'].
    pipeline: wgpu::ComputePipeline,
}

impl ComputeStage {
    pub fn new(device: &wgpu::Device, render_texture: &RenderTexture) -> Self {
        let shader = device.create_shader_module(&wgpu::include_spirv!("shader/comp.spv"));
        let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Compute Storage Buffer"),
            // There could be some performance gained by using the flag MAP_WRITE,
            // which maps the buffer directly to the CPU if the GPU
            // and CPU has shared memory. It's not supported on all systems however.
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

    /// Generate ['RenderTexture'] from the playstations VRAM. First it transfers the entire VRAM
    /// to the shdader, then it dispatches the compute shader for each pixel in ['RenderTexture'].
    pub fn compute_render_texture(
        &mut self,
        vram: &Vram,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        render_texture: &RenderTexture,
    ) {
        // Transfer the entire VRAM. This could be done with a staging belt, which should be faster
        // in theory. However in the testing i have done, that didn't seem to be the case, which
        // means that either write_buffer does the same under the hood, or it just isn't a
        // bottleneck. Perhaps it's faster on some systems, in which case it probably should be
        // used, but since it made the code more complicated, i opted not to use i it for now.
        queue.write_buffer(&self.input_buffer, 0, vram.raw_data());
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch(render_texture.extent.width, render_texture.extent.height, 1);
    }
}
