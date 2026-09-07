use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroup, Buffer, DepthStencilState, Device, Extent3d, FragmentState, LoadOp,
    MultisampleState, Operations, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPipeline, RenderPipelineDescriptor, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, VertexState,
};

use std::collections::BTreeMap;

use crate::{
    Camera3D, CameraProjection3D, Material3D, Node3DId, Primitive3D, ResolvedNode3D, Scene3D,
    SceneLighting3D,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
    material: [f32; 4],
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct Scene3DRenderer {
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    uniform_bind_group: BindGroup,
    resident_source: Option<ResidentSceneSource>,
    resident_mesh: Option<ResidentSceneMesh>,
    depth_texture: Texture,
    depth_view: TextureView,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SceneUniforms {
    view_projection: [[f32; 4]; 4],
    camera_position: [f32; 4],
    light_direction: [f32; 4],
    light_color: [f32; 4],
    lighting: [f32; 4],
}

impl SceneUniforms {
    fn new(camera: &Camera3D, lighting: &SceneLighting3D, aspect: f32) -> Self {
        let camera = effective_camera(camera);
        let lighting = effective_lighting(lighting);
        let light = lighting.directional;
        Self {
            view_projection: camera_view_projection(camera, aspect),
            camera_position: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
            light_direction: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            light_color: [
                light.color.r as f32 / 255.0,
                light.color.g as f32 / 255.0,
                light.color.b as f32 / 255.0,
                1.0,
            ],
            // Ambient, diffuse, specular strength, and specular exponent.
            lighting: [
                lighting.ambient_intensity,
                light.intensity,
                lighting.specular_intensity,
                lighting.specular_exponent,
            ],
        }
    }
}

struct ResidentSceneMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct ResidentSceneSource {
    primitives: Vec<Primitive3D>,
    nodes: Vec<ResolvedNode3D>,
    materials: BTreeMap<Node3DId, Material3D>,
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

        use wgpu::util::DeviceExt;
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fission-3d uniforms"),
            contents: bytemuck::bytes_of(&SceneUniforms {
                view_projection: glam::Mat4::IDENTITY.to_cols_array_2d(),
                camera_position: [0.0, 0.0, 1.0, 1.0],
                light_direction: [0.45, 0.8, 0.35, 0.0],
                light_color: [1.0, 0.98, 0.92, 1.0],
                lighting: [0.24, 0.82, 0.22, 32.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fission-3d uniforms bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
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
            uniform_buffer,
            uniform_bind_group,
            resident_source: None,
            resident_mesh: None,
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

        let uniforms = SceneUniforms::new(
            &scene.camera,
            &scene.lighting,
            (viewport.width / viewport.height).max(0.01),
        );
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.ensure_scene_mesh(device, scene);
        let Some(mesh) = self.resident_mesh.as_ref() else {
            return;
        };

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
            rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
            rpass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.0,
                1.0,
            );
            rpass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
            rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    fn ensure_scene_mesh(&mut self, device: &Device, scene: &Scene3D) {
        let nodes = scene.render_nodes();
        if resident_mesh_matches(
            self.resident_source.as_ref(),
            &scene.primitives,
            &nodes,
            &scene.materials,
        ) {
            return;
        }

        self.resident_mesh = build_scene_mesh(device, &scene.primitives, &nodes, &scene.materials);
        self.resident_source = Some(ResidentSceneSource {
            primitives: scene.primitives.clone(),
            nodes: nodes.into_owned(),
            materials: scene.materials.clone(),
        });
    }
}

fn resident_mesh_matches(
    cached: Option<&ResidentSceneSource>,
    primitives: &[Primitive3D],
    nodes: &[ResolvedNode3D],
    materials: &BTreeMap<Node3DId, Material3D>,
) -> bool {
    cached.is_some_and(|cached| {
        cached.primitives == primitives && cached.nodes == nodes && cached.materials == *materials
    })
}

fn build_scene_geometry(
    primitives: &[Primitive3D],
    nodes: &[ResolvedNode3D],
    materials: &BTreeMap<Node3DId, Material3D>,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for prim in primitives {
        push_primitive(
            &mut vertices,
            &mut indices,
            prim,
            glam::Mat4::IDENTITY,
            Material3D::default(),
        );
    }
    for node in nodes.iter().filter(|node| node.visible) {
        if let Some(primitive) = &node.primitive {
            push_primitive(
                &mut vertices,
                &mut indices,
                primitive,
                glam::Mat4::from_cols_array(&node.world_transform),
                materials.get(&node.id).copied().unwrap_or_default(),
            );
        }
    }

    (vertices, indices)
}

fn push_primitive(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    primitive: &Primitive3D,
    transform: glam::Mat4,
    material: Material3D,
) {
    let first_vertex = vertices.len();
    match primitive {
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
            push_cube(&mut vertices, &mut indices, p, color, material);
        }
        Primitive3D::Sphere {
            center,
            radius,
            color,
        } => {
            let base_idx = vertices.len() as u32;
            let c = [
                channel_product(color.r, material.tint.r),
                channel_product(color.g, material.tint.g),
                channel_product(color.b, material.tint.b),
                channel_product(color.a, material.tint.a),
            ];
            let surface = material_attributes(material);
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
                        normal: glam::Vec3::new(
                            phi.sin() * theta.cos(),
                            phi.cos(),
                            phi.sin() * theta.sin(),
                        )
                        .to_array(),
                        color: c,
                        material: surface,
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
            if v_in.is_empty()
                || i_in.is_empty()
                || i_in.len() % 3 != 0
                || i_in.iter().any(|index| (*index as usize) >= v_in.len())
            {
                return;
            }
            let base_idx = vertices.len() as u32;
            let c = [
                channel_product(color.r, material.tint.r),
                channel_product(color.g, material.tint.g),
                channel_product(color.b, material.tint.b),
                channel_product(color.a, material.tint.a),
            ];
            let surface = material_attributes(material);
            let mut normals = vec![glam::Vec3::ZERO; v_in.len()];
            for triangle in i_in.chunks_exact(3) {
                let a = point3_to_vec3(v_in[triangle[0] as usize]);
                let b = point3_to_vec3(v_in[triangle[1] as usize]);
                let c = point3_to_vec3(v_in[triangle[2] as usize]);
                let normal = (b - a).cross(c - a);
                for index in triangle {
                    normals[*index as usize] += normal;
                }
            }
            for (v, normal) in v_in.iter().zip(normals) {
                vertices.push(Vertex {
                    position: [v.x, v.y, v.z],
                    normal: normal.normalize_or_zero().to_array(),
                    color: c,
                    material: surface,
                });
            }
            for idx in i_in {
                indices.push(base_idx + *idx);
            }
        }
    }
    if transform != glam::Mat4::IDENTITY {
        let determinant = transform.determinant();
        let normal_matrix = (determinant.is_finite() && determinant.abs() > f32::EPSILON)
            .then(|| glam::Mat3::from_mat4(transform.inverse().transpose()));
        for vertex in &mut vertices[first_vertex..] {
            let position = transform.transform_point3(glam::Vec3::from_array(vertex.position));
            vertex.position = position.to_array();
            if let Some(normal_matrix) = normal_matrix {
                vertex.normal = normal_matrix
                    .mul_vec3(glam::Vec3::from_array(vertex.normal))
                    .normalize_or_zero()
                    .to_array();
            }
        }
    }
}

fn build_scene_mesh(
    device: &Device,
    primitives: &[Primitive3D],
    nodes: &[ResolvedNode3D],
    materials: &BTreeMap<Node3DId, Material3D>,
) -> Option<ResidentSceneMesh> {
    use wgpu::util::DeviceExt;
    let (vertices, indices) = build_scene_geometry(primitives, nodes, materials);
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("fission-3d vbuf"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("fission-3d ibuf"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    Some(ResidentSceneMesh {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    })
}

fn camera_view_projection(camera: &Camera3D, aspect: f32) -> [[f32; 4]; 4] {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let camera = effective_camera(camera);
    let eye = glam::Vec3::new(camera.eye.x, camera.eye.y, camera.eye.z);
    let target = glam::Vec3::new(camera.target.x, camera.target.y, camera.target.z);
    let up = glam::Vec3::new(camera.up.x, camera.up.y, camera.up.z).normalize();
    let view = glam::Mat4::look_at_rh(eye, target, up);
    let projection = match camera.projection {
        CameraProjection3D::Perspective {
            vertical_fov_radians,
            near,
            far,
        } => glam::Mat4::perspective_rh(vertical_fov_radians, aspect, near, far),
        CameraProjection3D::Orthographic {
            vertical_size,
            near,
            far,
        } => {
            let half_height = vertical_size * 0.5;
            let half_width = half_height * aspect;
            glam::Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                near,
                far,
            )
        }
    };
    (projection * view).to_cols_array_2d()
}

fn effective_camera(camera: &Camera3D) -> &Camera3D {
    if camera.is_valid() {
        camera
    } else {
        static DEFAULT_CAMERA: std::sync::LazyLock<Camera3D> =
            std::sync::LazyLock::new(Camera3D::default);
        &DEFAULT_CAMERA
    }
}

fn effective_lighting(lighting: &SceneLighting3D) -> &SceneLighting3D {
    if lighting.is_valid() {
        lighting
    } else {
        static DEFAULT_LIGHTING: std::sync::LazyLock<SceneLighting3D> =
            std::sync::LazyLock::new(SceneLighting3D::default);
        &DEFAULT_LIGHTING
    }
}

fn push_cube(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    p: [[f32; 3]; 8],
    color: &fission_core::op::Color,
    material: Material3D,
) {
    push_face(vertices, indices, [p[0], p[1], p[2], p[3]], color, material);
    push_face(vertices, indices, [p[5], p[4], p[7], p[6]], color, material);
    push_face(vertices, indices, [p[4], p[0], p[3], p[7]], color, material);
    push_face(vertices, indices, [p[1], p[5], p[6], p[2]], color, material);
    push_face(vertices, indices, [p[3], p[2], p[6], p[7]], color, material);
    push_face(vertices, indices, [p[4], p[5], p[1], p[0]], color, material);
}

fn push_face(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    positions: [[f32; 3]; 4],
    color: &fission_core::op::Color,
    material: Material3D,
) {
    let base_idx = vertices.len() as u32;
    let color = [
        channel_product(color.r, material.tint.r),
        channel_product(color.g, material.tint.g),
        channel_product(color.b, material.tint.b),
        channel_product(color.a, material.tint.a),
    ];
    let surface = material_attributes(material);
    let edge_a = glam::Vec3::from_array(positions[1]) - glam::Vec3::from_array(positions[0]);
    let edge_b = glam::Vec3::from_array(positions[2]) - glam::Vec3::from_array(positions[0]);
    // Cube faces predate back-face culling and use clockwise vertex order
    // when viewed from outside, so reverse the winding for outward normals.
    let normal = edge_b.cross(edge_a).normalize_or_zero().to_array();
    for position in positions {
        vertices.push(Vertex {
            position,
            normal,
            color,
            material: surface,
        });
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

fn channel_product(left: u8, right: u8) -> f32 {
    (left as f32 / 255.0) * (right as f32 / 255.0)
}

fn material_attributes(material: Material3D) -> [f32; 4] {
    [
        material.roughness,
        material.metallic,
        material.emissive_intensity,
        0.0,
    ]
}

fn point3_to_vec3(point: crate::Point3D) -> glam::Vec3 {
    glam::Vec3::new(point.x, point.y, point.z)
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
    use std::collections::BTreeMap;

    use super::{
        build_scene_geometry, camera_view_projection, clamp_scene3d_viewport, push_cube,
        resident_mesh_matches, ResidentSceneSource, Scene3DViewport,
    };
    use crate::{Camera3D, Material3D, Node3DId, Point3D, Primitive3D, ResolvedNode3D};
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
    fn cube_mesh_duplicates_faces_with_distinct_normals() {
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
            Material3D::default(),
        );

        assert_eq!(vertices.len(), 24);
        assert_eq!(indices.len(), 36);
        let first_face_normal = vertices[0].normal;
        assert_eq!(first_face_normal, [0.0, 0.0, -1.0]);
        assert!(vertices
            .iter()
            .any(|vertex| vertex.normal != first_face_normal));
    }

    #[test]
    fn resident_mesh_identity_changes_only_with_scene_geometry() {
        let cube = Primitive3D::Cube {
            center: Point3D::new(0.0, 0.0, 0.0),
            size: 2.0,
            color: Color::BLUE,
        };
        let cached_primitives = vec![cube.clone()];
        let cached = ResidentSceneSource {
            primitives: cached_primitives.clone(),
            nodes: Vec::new(),
            materials: BTreeMap::new(),
        };

        assert!(!resident_mesh_matches(
            None,
            &cached_primitives,
            &[],
            &BTreeMap::new()
        ));
        assert!(resident_mesh_matches(
            Some(&cached),
            &cached_primitives,
            &[],
            &BTreeMap::new()
        ));

        let changed = vec![Primitive3D::Cube { size: 3.0, ..cube }];
        assert!(!resident_mesh_matches(
            Some(&cached),
            &changed,
            &[],
            &BTreeMap::new()
        ));

        let changed_materials = BTreeMap::from([(
            Node3DId::explicit("material-change"),
            Material3D::new(Color::BLUE, 0.2, 0.7),
        )]);
        assert!(!resident_mesh_matches(
            Some(&cached),
            &cached_primitives,
            &[],
            &changed_materials,
        ));
    }

    #[test]
    fn empty_scene_geometry_is_cacheable_without_gpu_buffers() {
        let (vertices, indices) = build_scene_geometry(&[], &[], &BTreeMap::new());
        assert!(vertices.is_empty());
        assert!(indices.is_empty());
        let cached = ResidentSceneSource {
            primitives: Vec::new(),
            nodes: Vec::new(),
            materials: BTreeMap::new(),
        };
        assert!(resident_mesh_matches(
            Some(&cached),
            &[],
            &[],
            &BTreeMap::new()
        ));
    }

    #[test]
    fn resolved_node_transform_is_applied_to_generated_geometry() {
        let node = ResolvedNode3D {
            id: Node3DId::explicit("translated-cube"),
            world_transform: glam::Mat4::from_translation(glam::vec3(5.0, 0.0, 0.0))
                .to_cols_array(),
            primitive: Some(Primitive3D::Cube {
                center: Point3D::new(0.0, 0.0, 0.0),
                size: 2.0,
                color: Color::BLUE,
            }),
            visible: true,
        };

        let (vertices, indices) = build_scene_geometry(&[], &[node], &BTreeMap::new());

        assert_eq!(vertices.len(), 24);
        assert_eq!(indices.len(), 36);
        assert!(vertices
            .iter()
            .all(|vertex| (4.0..=6.0).contains(&vertex.position[0])));
    }

    #[test]
    fn retained_material_tints_geometry_and_changes_surface_attributes() {
        let id = Node3DId::explicit("water-material");
        let node = ResolvedNode3D {
            id,
            world_transform: glam::Mat4::IDENTITY.to_cols_array(),
            primitive: Some(Primitive3D::Sphere {
                center: Point3D::new(0.0, 0.0, 0.0),
                radius: 1.0,
                color: Color::WHITE,
            }),
            visible: true,
        };
        let material = Material3D::new(Color::BLUE, 0.08, 0.18).emissive(0.03);
        let materials = BTreeMap::from([(id, material)]);

        let (vertices, indices) = build_scene_geometry(&[], &[node], &materials);

        assert!(!indices.is_empty());
        assert!(!vertices.is_empty());
        assert_eq!(vertices[0].color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(vertices[0].material, [0.08, 0.18, 0.03, 0.0]);
    }

    #[test]
    fn perspective_and_orthographic_camera_matrices_are_finite() {
        let perspective = camera_view_projection(&Camera3D::default(), 16.0 / 9.0);
        let orthographic = camera_view_projection(
            &Camera3D::orthographic(
                Point3D::new(0.0, 4.0, 8.0),
                Point3D::new(0.0, 0.0, 0.0),
                12.0,
                0.1,
                100.0,
            ),
            16.0 / 9.0,
        );

        assert!(perspective.into_iter().flatten().all(f32::is_finite));
        assert!(orthographic.into_iter().flatten().all(f32::is_finite));
    }
}
