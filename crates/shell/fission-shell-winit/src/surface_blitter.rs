use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, ColorTargetState,
    ColorWrites, CommandEncoder, Device, FilterMode, FragmentState, FrontFace, LoadOp,
    MultisampleState, PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState,
    PrimitiveTopology, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StoreOp, TextureFormat, TextureSampleType, TextureView, TextureViewDimension, VertexState,
};

/// Copies the stable intermediate frame target to the current surface texture.
///
/// The source bind group only changes when the intermediate target is recreated,
/// so retain it between frames instead of allocating one during every blit.
pub(crate) struct SurfaceBlitter {
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    sampler: Sampler,
    source_bind_group: BindGroup,
}

impl SurfaceBlitter {
    pub(crate) fn new(
        device: &Device,
        format: TextureFormat,
        source: &TextureView,
        blend: Option<BlendState>,
    ) -> Self {
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Fission surface blitter sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Fission surface blitter bind group layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Fission surface blitter pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Fission surface blitter shader"),
            source: ShaderSource::Wgsl(include_str!("surface_blitter.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Fission surface blitter pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format,
                    blend,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let source_bind_group =
            Self::create_source_bind_group(device, &bind_group_layout, &sampler, source);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            source_bind_group,
        }
    }

    pub(crate) fn update_source(&mut self, device: &Device, source: &TextureView) {
        self.source_bind_group =
            Self::create_source_bind_group(device, &self.bind_group_layout, &self.sampler, source);
    }

    pub(crate) fn copy(&self, encoder: &mut CommandEncoder, target: &TextureView) {
        crate::record_web_test_diagnostic(509, 0);
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Fission surface blit pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        crate::record_web_test_diagnostic(510, 0);
        pass.set_pipeline(&self.pipeline);
        crate::record_web_test_diagnostic(511, 0);
        pass.set_bind_group(0, &self.source_bind_group, &[]);
        crate::record_web_test_diagnostic(512, 0);
        pass.draw(0..3, 0..1);
        crate::record_web_test_diagnostic(513, 0);
        drop(pass);
        crate::record_web_test_diagnostic(514, 0);
    }

    fn create_source_bind_group(
        device: &Device,
        layout: &BindGroupLayout,
        sampler: &Sampler,
        source: &TextureView,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Fission surface blitter source bind group"),
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}
