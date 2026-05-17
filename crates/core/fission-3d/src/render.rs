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
    uniform_layout: wgpu::BindGroupLayout,
    depth_texture: Texture,
    depth_view: TextureView,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SceneUniforms {
    aspect: f32,
    _pad: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scene3DViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Scene3DRenderer {
    pub fn new(device: &Device, width: u32, height: u32, target_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fission-3d shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fission-3d uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("fission-3d layout"),
            bind_group_layouts: &[&uniform_layout],
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
                cull_mode: None,
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
            uniform_layout,
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
        self.render_in_rect(
            device,
            queue,
            view,
            scene,
            Scene3DViewport {
                x: 0.0,
                y: 0.0,
                width: self.width as f32,
                height: self.height as f32,
            },
        );
    }

    pub fn render_in_rect(
        &mut self,
        device: &Device,
        queue: &Queue,
        view: &TextureView,
        scene: &Scene3D,
        viewport: Scene3DViewport,
    ) {
        let Some((viewport, scissor)) = clamp_scene3d_viewport(viewport, self.width, self.height)
        else {
            return;
        };

        use wgpu::util::DeviceExt;

        let uniforms = SceneUniforms {
            aspect: (viewport.width / viewport.height).max(0.01),
            _pad: [0.0; 3],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fission-3d uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fission-3d uniforms bind group"),
            layout: &self.uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

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
                    push_cube(&mut vertices, &mut indices, p, color);
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
            rpass.set_bind_group(0, &uniform_bind_group, &[]);
            rpass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.0,
                1.0,
            );
            rpass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
            rpass.set_vertex_buffer(0, v_buf.slice(..));
            rpass.set_index_buffer(i_buf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}

fn push_cube(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    p: [[f32; 3]; 8],
    color: &fission_core::op::Color,
) {
    push_face(vertices, indices, [p[0], p[1], p[2], p[3]], color, 0.86);
    push_face(vertices, indices, [p[5], p[4], p[7], p[6]], color, 0.64);
    push_face(vertices, indices, [p[4], p[0], p[3], p[7]], color, 0.72);
    push_face(vertices, indices, [p[1], p[5], p[6], p[2]], color, 1.0);
    push_face(vertices, indices, [p[3], p[2], p[6], p[7]], color, 1.18);
    push_face(vertices, indices, [p[4], p[5], p[1], p[0]], color, 0.52);
}

fn push_face(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    positions: [[f32; 3]; 4],
    color: &fission_core::op::Color,
    shade: f32,
) {
    let base_idx = vertices.len() as u32;
    let color = shaded_color(color, shade);
    for position in positions {
        vertices.push(Vertex { position, color });
    }
    indices.extend_from_slice(&[
        base_idx,
        base_idx + 1,
        base_idx + 2,
        base_idx,
        base_idx + 2,
        base_idx + 3,
    ]);
}

fn shaded_color(color: &fission_core::op::Color, shade: f32) -> [f32; 4] {
    [
        ((color.r as f32 / 255.0) * shade).clamp(0.0, 1.0),
        ((color.g as f32 / 255.0) * shade).clamp(0.0, 1.0),
        ((color.b as f32 / 255.0) * shade).clamp(0.0, 1.0),
        color.a as f32 / 255.0,
    ]
}

fn clamp_scene3d_viewport(
    viewport: Scene3DViewport,
    target_width: u32,
    target_height: u32,
) -> Option<(Scene3DViewport, (u32, u32, u32, u32))> {
    if target_width == 0
        || target_height == 0
        || !viewport.x.is_finite()
        || !viewport.y.is_finite()
        || !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
    {
        return None;
    }

    let target_width_f = target_width as f32;
    let target_height_f = target_height as f32;
    let x0 = viewport.x.max(0.0).min(target_width_f);
    let y0 = viewport.y.max(0.0).min(target_height_f);
    let x1 = (viewport.x + viewport.width).max(0.0).min(target_width_f);
    let y1 = (viewport.y + viewport.height).max(0.0).min(target_height_f);

    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    let scissor_x = x0.floor() as u32;
    let scissor_y = y0.floor() as u32;
    let scissor_right = (x1.ceil() as u32).min(target_width);
    let scissor_bottom = (y1.ceil() as u32).min(target_height);
    let scissor_width = scissor_right.saturating_sub(scissor_x);
    let scissor_height = scissor_bottom.saturating_sub(scissor_y);

    if scissor_width == 0 || scissor_height == 0 {
        return None;
    }

    Some((
        Scene3DViewport {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        },
        (scissor_x, scissor_y, scissor_width, scissor_height),
    ))
}

#[cfg(test)]
mod tests {
    use super::{clamp_scene3d_viewport, push_cube, Scene3DViewport};
    use fission_core::op::Color;

    #[test]
    fn viewport_clamps_to_render_target() {
        let (viewport, scissor) = clamp_scene3d_viewport(
            Scene3DViewport {
                x: -10.0,
                y: 20.25,
                width: 130.0,
                height: 90.0,
            },
            100,
            80,
        )
        .expect("viewport should intersect target");

        assert_eq!(
            viewport,
            Scene3DViewport {
                x: 0.0,
                y: 20.25,
                width: 100.0,
                height: 59.75,
            }
        );
        assert_eq!(scissor, (0, 20, 100, 60));
    }

    #[test]
    fn viewport_outside_target_is_skipped() {
        assert!(clamp_scene3d_viewport(
            Scene3DViewport {
                x: 120.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            100,
            80,
        )
        .is_none());
    }

    #[test]
    fn cube_mesh_duplicates_faces_with_shading() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let p = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];

        push_cube(
            &mut vertices,
            &mut indices,
            p,
            &Color {
                r: 20,
                g: 184,
                b: 166,
                a: 255,
            },
        );

        assert_eq!(vertices.len(), 24);
        assert_eq!(indices.len(), 36);
        let first_face_color = vertices[0].color;
        assert!(vertices
            .iter()
            .any(|vertex| vertex.color != first_face_color));
    }
}
