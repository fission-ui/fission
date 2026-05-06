use bytemuck::{Pod, Zeroable};
use wgpu::{
    DepthStencilState, Device, Extent3d, FragmentState, LoadOp, MultisampleState, Operations,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPipeline,
    RenderPipelineDescriptor, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor, VertexState,
};

use crate::{Primitive3D, Scene3D};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct Scene3DRenderer {
    pipeline: RenderPipeline,
    depth_texture: Texture,
    depth_view: TextureView,
    width: u32,
    height: u32,
}

impl Scene3DRenderer {
    pub fn new(device: &Device, width: u32, height: u32, target_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fission-3d shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("fission-3d layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("fission-3d pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("fission-3d depth"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

        Self {
            pipeline,
            depth_texture,
            depth_view,
            width,
            height,
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;

        self.depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("fission-3d depth"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.depth_view = self
            .depth_texture
            .create_view(&TextureViewDescriptor::default());
    }

    pub fn render(&mut self, device: &Device, queue: &Queue, view: &TextureView, scene: &Scene3D) {
        // Construct mesh for primitives
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        // This is a naive tessellator just for demonstration parity.
        // It maps standard Scene3D primitives into flat TriangleLists.
        for prim in &scene.primitives {
            match prim {
                Primitive3D::Cube {
                    center,
                    size,
                    color,
                } => {
                    let hs = size / 2.0;
                    let (x, y, z) = (center.x, center.y, center.z);
                    let base_idx = vertices.len() as u32;
                    let c = [
                        color.r as f32 / 255.0,
                        color.g as f32 / 255.0,
                        color.b as f32 / 255.0,
                        color.a as f32 / 255.0,
                    ];

                    let p = [
                        [x - hs, y - hs, z - hs],
                        [x + hs, y - hs, z - hs],
                        [x + hs, y + hs, z - hs],
                        [x - hs, y + hs, z - hs],
                        [x - hs, y - hs, z + hs],
                        [x + hs, y - hs, z + hs],
                        [x + hs, y + hs, z + hs],
                        [x - hs, y + hs, z + hs],
                    ];

                    for pos in p {
                        vertices.push(Vertex {
                            position: pos,
                            color: c,
                        });
                    }

                    // Front
                    indices.extend_from_slice(&[
                        base_idx,
                        base_idx + 1,
                        base_idx + 2,
                        base_idx,
                        base_idx + 2,
                        base_idx + 3,
                    ]);
                    // Back
                    indices.extend_from_slice(&[
                        base_idx + 5,
                        base_idx + 4,
                        base_idx + 7,
                        base_idx + 5,
                        base_idx + 7,
                        base_idx + 6,
                    ]);
                    // Left
                    indices.extend_from_slice(&[
                        base_idx + 4,
                        base_idx,
                        base_idx + 3,
                        base_idx + 4,
                        base_idx + 3,
                        base_idx + 7,
                    ]);
                    // Right
                    indices.extend_from_slice(&[
                        base_idx + 1,
                        base_idx + 5,
                        base_idx + 6,
                        base_idx + 1,
                        base_idx + 6,
                        base_idx + 2,
                    ]);
                    // Top
                    indices.extend_from_slice(&[
                        base_idx + 3,
                        base_idx + 2,
                        base_idx + 6,
                        base_idx + 3,
                        base_idx + 6,
                        base_idx + 7,
                    ]);
                    // Bottom
                    indices.extend_from_slice(&[
                        base_idx + 4,
                        base_idx + 5,
                        base_idx + 1,
                        base_idx + 4,
                        base_idx + 1,
                        base_idx,
                    ]);
                }
                Primitive3D::Sphere {
                    center,
                    radius,
                    color,
                } => {
                    let base_idx = vertices.len() as u32;
                    let c = [
                        color.r as f32 / 255.0,
                        color.g as f32 / 255.0,
                        color.b as f32 / 255.0,
                        color.a as f32 / 255.0,
                    ];
                    let segments = 16;
                    let rings = 16;

                    for i in 0..=rings {
                        let v = i as f32 / rings as f32;
                        let phi = v * std::f32::consts::PI;

                        for j in 0..=segments {
                            let u = j as f32 / segments as f32;
                            let theta = u * std::f32::consts::PI * 2.0;

                            let x = center.x + radius * phi.sin() * theta.cos();
                            let y = center.y + radius * phi.cos();
                            let z = center.z + radius * phi.sin() * theta.sin();

                            vertices.push(Vertex {
                                position: [x, y, z],
                                color: c,
                            });
                        }
                    }

                    for i in 0..rings {
                        for j in 0..segments {
                            let first = base_idx + (i * (segments + 1)) as u32 + j as u32;
                            let second = first + segments as u32 + 1;

                            indices.push(first);
                            indices.push(second);
                            indices.push(first + 1);

                            indices.push(second);
                            indices.push(second + 1);
                            indices.push(first + 1);
                        }
                    }
                }
                Primitive3D::Mesh {
                    vertices: v_in,
                    indices: i_in,
                    color,
                } => {
                    let base_idx = vertices.len() as u32;
                    let c = [
                        color.r as f32 / 255.0,
                        color.g as f32 / 255.0,
                        color.b as f32 / 255.0,
                        color.a as f32 / 255.0,
                    ];
                    for v in v_in {
                        vertices.push(Vertex {
                            position: [v.x, v.y, v.z],
                            color: c,
                        });
                    }
                    for idx in i_in {
                        indices.push(base_idx + *idx);
                    }
                }
            }
        }

        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        use wgpu::util::DeviceExt;
        let v_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fission-3d vbuf"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let i_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fission-3d ibuf"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fission-3d enc"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fission-3d pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_vertex_buffer(0, v_buf.slice(..));
            rpass.set_index_buffer(i_buf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
