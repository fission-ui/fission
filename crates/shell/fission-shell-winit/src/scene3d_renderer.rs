//! Persistent wgpu renderer for the closed `fission-scene3d` packet.

use std::collections::{HashMap, HashSet};

use bytemuck::{Pod, Zeroable};
use fission_scene3d::{
    AlphaMode3D, CameraProjection3D, CullMode3D, DrawCommand3D, Light3D, Material3D,
    MaterialModel3D, Mesh3D, MeshId, MeshVertex3D, NodeContent3D, PreparedScene3D, Primitive3D,
    ResourceId, Scene3DRenderPacket, TextureId, TextureSampling3D,
};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Scene3DViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Scene3DRenderDiagnosticKind {
    InvalidPacket,
    InvalidScene,
    UnsupportedPacket,
    MissingTextureAsset,
    WrongTextureAssetKind,
    TextureLoading,
    TextureUnreadable,
    TextureDecodeFailed,
    RendererLimit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scene3DRenderDiagnostic {
    pub kind: Scene3DRenderDiagnosticKind,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Scene3DRenderReport {
    pub draw_calls: u32,
    pub triangles: u32,
    pub uploaded_meshes: u32,
    pub uploaded_textures: u32,
    pub reused_resources: u32,
    /// Every referenced resource has either loaded or reached a typed failure.
    pub ready: bool,
    pub meaningful_content: bool,
    pub first_meaningful_content: bool,
    pub diagnostics: Vec<Scene3DRenderDiagnostic>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}

impl Vertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SceneUniforms {
    view_projection: [[f32; 4]; 4],
    camera_position: [f32; 4],
    ambient: [f32; 4],
    directional_color: [f32; 4],
    directional_direction: [f32; 4],
    point_position_range: [[f32; 4]; 4],
    point_color_intensity: [[f32; 4]; 4],
    point_count: [u32; 4],
}

unsafe impl Zeroable for SceneUniforms {}
unsafe impl Pod for SceneUniforms {}

#[repr(C)]
#[derive(Clone, Copy)]
struct TransformUniforms {
    model: [[f32; 4]; 4],
    normal_model: [[f32; 4]; 4],
}

unsafe impl Zeroable for TransformUniforms {}
unsafe impl Pod for TransformUniforms {}

#[repr(C)]
#[derive(Clone, Copy)]
struct MaterialUniforms {
    base_color: [f32; 4],
    emissive: [f32; 4],
    factors: [f32; 4],
    flags: [u32; 4],
}

unsafe impl Zeroable for MaterialUniforms {}
unsafe impl Pod for MaterialUniforms {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum MeshCacheKey {
    Cube,
    Sphere,
    Resource(u64, MeshId, u64),
}

struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TextureCacheKey {
    scene: u64,
    asset: u64,
    revision: u64,
}

struct GpuTexture {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaterialCacheKey {
    scene: u64,
    id: u64,
    default_material: bool,
    revision: u64,
    base_texture: Option<TextureCacheKey>,
    emissive_texture: Option<TextureCacheKey>,
}

struct GpuMaterial {
    #[allow(dead_code)]
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PipelineKey {
    alpha: AlphaPipeline,
    cull: CullMode3D,
    depth_test: bool,
    depth_write: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AlphaPipeline {
    Opaque,
    Blend,
}

struct QueuedDraw {
    mesh_key: MeshCacheKey,
    material_key: MaterialCacheKey,
    pipeline_key: PipelineKey,
    model: Mat4,
}

struct FrameDraw {
    mesh_key: MeshCacheKey,
    material_key: MaterialCacheKey,
    pipeline_key: PipelineKey,
    #[allow(dead_code)]
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
}

pub(crate) struct Scene3DRenderer {
    target_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    scene_layout: wgpu::BindGroupLayout,
    transform_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    clear_layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    clear_pipeline: wgpu::RenderPipeline,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    meshes: HashMap<MeshCacheKey, GpuMesh>,
    textures: HashMap<TextureCacheKey, GpuTexture>,
    materials: HashMap<MaterialCacheKey, GpuMaterial>,
    failed_textures: HashSet<TextureCacheKey>,
    fallback_texture: GpuTexture,
    reported_first_content: bool,
}

impl Scene3DRenderer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let scene_layout = uniform_layout(
            device,
            "scene3d scene uniforms",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        let transform_layout = uniform_layout(
            device,
            "scene3d transform uniforms",
            wgpu::ShaderStages::VERTEX,
        );
        let clear_layout = uniform_layout(
            device,
            "scene3d clear uniforms",
            wgpu::ShaderStages::FRAGMENT,
        );
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene3d material"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_layout_entry(1),
                texture_layout_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene3d shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene3d.wgsl").into()),
        });
        let clear_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene3d clear shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene3d_clear.wgsl").into()),
        });
        let clear_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scene3d clear pipeline layout"),
                bind_group_layouts: &[Some(&clear_layout)],
                immediate_size: 0,
            });
        let clear_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene3d clear pipeline"),
            layout: Some(&clear_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &clear_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &clear_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let (depth_texture, depth_view) = depth_target(device, width, height);
        let fallback_texture = upload_rgba_texture(
            device,
            queue,
            "scene3d deliberate white texture fallback",
            1,
            1,
            &[255, 255, 255, 255],
            true,
        );
        Self {
            target_format,
            width,
            height,
            depth_texture,
            depth_view,
            scene_layout,
            transform_layout,
            material_layout,
            clear_layout,
            shader,
            clear_pipeline,
            pipelines: HashMap::new(),
            meshes: HashMap::new(),
            textures: HashMap::new(),
            materials: HashMap::new(),
            failed_textures: HashSet::new(),
            fallback_texture,
            reported_first_content: false,
        }
    }

    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        (self.depth_texture, self.depth_view) = depth_target(device, width, height);
    }

    pub(crate) fn render_packet(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        payload: &[u8],
        viewport: Scene3DViewport,
    ) -> Scene3DRenderReport {
        match Scene3DRenderPacket::decode(payload) {
            Ok(packet) => self.render_prepared(device, queue, target, &packet.prepared, viewport),
            Err(message) => {
                let kind = if payload.starts_with(fission_scene3d::SCENE3D_EMBED_MAGIC) {
                    Scene3DRenderDiagnosticKind::InvalidPacket
                } else {
                    Scene3DRenderDiagnosticKind::UnsupportedPacket
                };
                let mut report = Scene3DRenderReport {
                    ready: true,
                    ..Scene3DRenderReport::default()
                };
                report
                    .diagnostics
                    .push(Scene3DRenderDiagnostic { kind, message });
                self.render_error_surface(device, queue, target, viewport);
                report
            }
        }
    }

    pub(crate) fn render_prepared(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        prepared: &PreparedScene3D,
        viewport: Scene3DViewport,
    ) -> Scene3DRenderReport {
        let Some((viewport, scissor)) = clamp_viewport(viewport, self.width, self.height) else {
            return Scene3DRenderReport::default();
        };
        let mut report = Scene3DRenderReport {
            ready: true,
            ..Scene3DRenderReport::default()
        };
        if !prepared.is_renderable() {
            report.diagnostics.extend(
                prepared
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == fission_scene::SceneSeverity::Error)
                    .map(|diagnostic| Scene3DRenderDiagnostic {
                        kind: Scene3DRenderDiagnosticKind::InvalidScene,
                        message: format!("{}: {}", diagnostic.code, diagnostic.message),
                    }),
            );
            if report.diagnostics.is_empty() {
                report.diagnostics.push(Scene3DRenderDiagnostic {
                    kind: Scene3DRenderDiagnosticKind::InvalidScene,
                    message: "scene contains no renderable content".into(),
                });
            }
            self.render_error_surface(device, queue, target, viewport);
            return report;
        }
        let point_lights = prepared
            .source
            .lights
            .iter()
            .filter(|light| matches!(light, Light3D::Point(_)))
            .count();
        if point_lights > 4 {
            report.diagnostics.push(Scene3DRenderDiagnostic {
                kind: Scene3DRenderDiagnosticKind::RendererLimit,
                message: format!(
                    "scene declares {point_lights} point lights; this renderer uses the first 4"
                ),
            });
        }
        let scene_uniforms = make_scene_uniforms(prepared, viewport);
        let scene_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene3d scene uniforms"),
            contents: bytemuck::bytes_of(&scene_uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene3d scene bind group"),
            layout: &self.scene_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_buffer.as_entire_binding(),
            }],
        });
        let queued = self.prepare_draws(device, queue, prepared, &mut report);
        let frame_draws = queued
            .into_iter()
            .map(|draw| {
                let normal = draw.model.inverse().transpose();
                let uniforms = TransformUniforms {
                    model: draw.model.to_cols_array_2d(),
                    normal_model: normal.to_cols_array_2d(),
                };
                let transform_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("scene3d transform uniforms"),
                        contents: bytemuck::bytes_of(&uniforms),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("scene3d transform bind group"),
                    layout: &self.transform_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: transform_buffer.as_entire_binding(),
                    }],
                });
                FrameDraw {
                    mesh_key: draw.mesh_key,
                    material_key: draw.material_key,
                    pipeline_key: draw.pipeline_key,
                    transform_buffer,
                    transform_bind_group,
                }
            })
            .collect::<Vec<_>>();
        let clear = prepared.source.clear.color;
        self.clear_rect(
            device,
            queue,
            target,
            viewport,
            scissor,
            [clear.r, clear.g, clear.b, clear.a],
        );
        if frame_draws.is_empty() {
            return report;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scene3d render encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene3d render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: prepared
                            .source
                            .clear
                            .depth
                            .map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
            pass.set_bind_group(0, &scene_bind_group, &[]);
            for draw in &frame_draws {
                let mesh = &self.meshes[&draw.mesh_key];
                let material = &self.materials[&draw.material_key];
                pass.set_pipeline(&self.pipelines[&draw.pipeline_key]);
                pass.set_bind_group(1, &draw.transform_bind_group, &[]);
                pass.set_bind_group(2, &material.bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                report.draw_calls += 1;
                report.triangles += mesh.index_count / 3;
            }
        }
        queue.submit(Some(encoder.finish()));
        report.meaningful_content = report.draw_calls > 0;
        if report.meaningful_content && !self.reported_first_content {
            self.reported_first_content = true;
            report.first_meaningful_content = true;
        }
        report
    }

    fn prepare_draws(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &PreparedScene3D,
        report: &mut Scene3DRenderReport,
    ) -> Vec<QueuedDraw> {
        let mut queued = Vec::new();
        for draw in &prepared.draws {
            match draw.content {
                NodeContent3D::Group => {}
                NodeContent3D::Primitive(primitive) => {
                    self.queue_primitive(
                        device,
                        queue,
                        prepared,
                        draw,
                        primitive,
                        report,
                        &mut queued,
                    );
                }
                NodeContent3D::Model { model } => {
                    let Some(model_resource) = prepared.source.resources.models.get(&model) else {
                        continue;
                    };
                    let mut model_world = Vec::<Mat4>::with_capacity(model_resource.nodes.len());
                    for model_node in &model_resource.nodes {
                        let local = transform_matrix(model_node.transform);
                        let world = model_node
                            .parent
                            .and_then(|parent| model_world.get(parent as usize).copied())
                            .unwrap_or(Mat4::IDENTITY)
                            * local;
                        model_world.push(world);
                        if !model_node.visible {
                            continue;
                        }
                        let Some(mesh) = model_node.mesh else {
                            continue;
                        };
                        let material = draw.material.or(model_node.material);
                        self.queue_mesh(
                            device,
                            queue,
                            prepared,
                            mesh,
                            material,
                            Mat4::from_cols_array(&draw.world_transform) * world,
                            report,
                            &mut queued,
                        );
                    }
                }
            }
        }
        queued
    }

    #[allow(clippy::too_many_arguments)]
    fn queue_primitive(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &PreparedScene3D,
        draw: &DrawCommand3D,
        primitive: Primitive3D,
        report: &mut Scene3DRenderReport,
        queued: &mut Vec<QueuedDraw>,
    ) {
        let base = Mat4::from_cols_array(&draw.world_transform);
        match primitive {
            Primitive3D::Cube { size } => {
                self.ensure_builtin_mesh(device, MeshCacheKey::Cube, report);
                self.queue_cached_mesh(
                    device,
                    queue,
                    prepared,
                    MeshCacheKey::Cube,
                    draw.material,
                    base * Mat4::from_scale(Vec3::new(size.x, size.y, size.z)),
                    report,
                    queued,
                );
            }
            Primitive3D::Sphere { radius } => {
                self.ensure_builtin_mesh(device, MeshCacheKey::Sphere, report);
                self.queue_cached_mesh(
                    device,
                    queue,
                    prepared,
                    MeshCacheKey::Sphere,
                    draw.material,
                    base * Mat4::from_scale(Vec3::splat(radius)),
                    report,
                    queued,
                );
            }
            Primitive3D::Mesh { mesh } => {
                self.queue_mesh(
                    device,
                    queue,
                    prepared,
                    mesh,
                    draw.material,
                    base,
                    report,
                    queued,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn queue_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &PreparedScene3D,
        mesh_id: MeshId,
        material_id: Option<ResourceId>,
        model: Mat4,
        report: &mut Scene3DRenderReport,
        queued: &mut Vec<QueuedDraw>,
    ) {
        let Some(mesh) = prepared.source.resources.meshes.get(&mesh_id) else {
            return;
        };
        let key = MeshCacheKey::Resource(prepared.source.id.get(), mesh_id, mesh.revision);
        if self.meshes.contains_key(&key) {
            report.reused_resources += 1;
        } else {
            self.meshes.insert(key, upload_mesh(device, mesh));
            report.uploaded_meshes += 1;
        }
        self.queue_cached_mesh(
            device,
            queue,
            prepared,
            key,
            material_id,
            model,
            report,
            queued,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn queue_cached_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &PreparedScene3D,
        mesh_key: MeshCacheKey,
        material_id: Option<ResourceId>,
        model: Mat4,
        report: &mut Scene3DRenderReport,
        queued: &mut Vec<QueuedDraw>,
    ) {
        let (material_id, material, default_material) = material_id
            .and_then(|id| {
                prepared
                    .source
                    .resources
                    .materials
                    .get(&id)
                    .map(|material| (id, material, false))
            })
            .unwrap_or((ResourceId(0), &DEFAULT_MATERIAL, true));
        let material_key = self.ensure_material(
            device,
            queue,
            prepared,
            material_id,
            material,
            default_material,
            report,
        );
        let alpha = if material.alpha_mode == AlphaMode3D::Blend {
            AlphaPipeline::Blend
        } else {
            AlphaPipeline::Opaque
        };
        let pipeline_key = PipelineKey {
            alpha,
            cull: if material.double_sided {
                CullMode3D::None
            } else {
                prepared.source.cull_mode
            },
            depth_test: prepared.source.depth.test,
            depth_write: prepared.source.depth.write && alpha == AlphaPipeline::Opaque,
        };
        self.ensure_pipeline(device, pipeline_key);
        queued.push(QueuedDraw {
            mesh_key,
            material_key,
            pipeline_key,
            model,
        });
    }

    fn ensure_builtin_mesh(
        &mut self,
        device: &wgpu::Device,
        key: MeshCacheKey,
        report: &mut Scene3DRenderReport,
    ) {
        if self.meshes.contains_key(&key) {
            report.reused_resources += 1;
            return;
        }
        let (vertices, indices) = match key {
            MeshCacheKey::Cube => cube_geometry(),
            MeshCacheKey::Sphere => sphere_geometry(24, 16),
            MeshCacheKey::Resource(..) => unreachable!(),
        };
        self.meshes
            .insert(key, upload_raw_mesh(device, &vertices, &indices));
        report.uploaded_meshes += 1;
    }

    fn ensure_material(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &PreparedScene3D,
        id: ResourceId,
        material: &Material3D,
        default_material: bool,
        report: &mut Scene3DRenderReport,
    ) -> MaterialCacheKey {
        let base_key =
            self.ensure_texture(device, queue, prepared, material.base_color_texture, report);
        let emissive_key =
            self.ensure_texture(device, queue, prepared, material.emissive_texture, report);
        let key = MaterialCacheKey {
            scene: prepared.source.id.get(),
            id: id.0,
            default_material,
            revision: material.revision,
            base_texture: base_key,
            emissive_texture: emissive_key,
        };
        if self.materials.contains_key(&key) {
            report.reused_resources += 1;
            return key;
        }
        let flags = (matches!(material.model, MaterialModel3D::Unlit) as u32)
            | ((material.base_color_texture.is_some() && base_key.is_some()) as u32) << 1
            | ((material.emissive_texture.is_some() && emissive_key.is_some()) as u32) << 2
            | ((material.alpha_mode == AlphaMode3D::Mask) as u32) << 3;
        let (metallic, roughness) = match material.model {
            MaterialModel3D::Unlit => (0.0, 1.0),
            MaterialModel3D::MetallicRoughness {
                metallic,
                roughness,
            } => (metallic, roughness),
        };
        let uniforms = MaterialUniforms {
            base_color: rgba(material.base_color),
            emissive: rgba(material.emissive_color),
            factors: [metallic, roughness, material.alpha_cutoff, 0.0],
            flags: [flags, 0, 0, 0],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene3d material uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let base = base_key
            .and_then(|key| self.textures.get(&key))
            .unwrap_or(&self.fallback_texture);
        let emissive = emissive_key
            .and_then(|key| self.textures.get(&key))
            .unwrap_or(&self.fallback_texture);
        let base_sampling = material
            .base_color_texture
            .and_then(|id| prepared.source.resources.textures.get(&id))
            .map_or(TextureSampling3D::Linear, |texture| texture.sampling);
        let base_sampler = match base_sampling {
            TextureSampling3D::Nearest => &base.nearest,
            TextureSampling3D::Linear => &base.linear,
        };
        let emissive_sampling = material
            .emissive_texture
            .and_then(|id| prepared.source.resources.textures.get(&id))
            .map_or(TextureSampling3D::Linear, |texture| texture.sampling);
        let emissive_sampler = match emissive_sampling {
            TextureSampling3D::Nearest => &emissive.nearest,
            TextureSampling3D::Linear => &emissive.linear,
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene3d material bind group"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&base.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&emissive.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(base_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(emissive_sampler),
                },
            ],
        });
        self.materials.insert(
            key,
            GpuMaterial {
                uniform,
                bind_group,
            },
        );
        key
    }

    fn ensure_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &PreparedScene3D,
        texture_id: Option<TextureId>,
        report: &mut Scene3DRenderReport,
    ) -> Option<TextureCacheKey> {
        let id = texture_id?;
        let texture = prepared.source.resources.textures.get(&id)?;
        let key = TextureCacheKey {
            scene: prepared.source.id.get(),
            asset: texture.asset.id().get(),
            revision: texture.revision,
        };
        if self.textures.contains_key(&key) {
            report.reused_resources += 1;
            return Some(key);
        }
        let Some(descriptor) = prepared
            .source
            .assets
            .assets
            .iter()
            .find(|entry| entry.id == texture.asset.id())
        else {
            self.failed_textures.insert(key);
            report.diagnostics.push(Scene3DRenderDiagnostic {
                kind: Scene3DRenderDiagnosticKind::MissingTextureAsset,
                message: format!("texture {} references missing asset {}", id.0, key.asset),
            });
            return None;
        };
        if !matches!(
            descriptor.kind,
            fission_scene::AssetKind::Image | fission_scene::AssetKind::Texture
        ) {
            self.failed_textures.insert(key);
            report.diagnostics.push(Scene3DRenderDiagnostic {
                kind: Scene3DRenderDiagnosticKind::WrongTextureAssetKind,
                message: format!(
                    "texture {} asset {} has kind {:?}",
                    id.0, key.asset, descriptor.kind
                ),
            });
            return None;
        }
        if self.failed_textures.contains(&key) {
            return None;
        }
        match load_asset_bytes(&descriptor.source) {
            AssetBytes::Ready(bytes) => match image::load_from_memory(&bytes) {
                Ok(image) => {
                    let image = image.to_rgba8();
                    let gpu = upload_rgba_texture(
                        device,
                        queue,
                        "scene3d packaged texture",
                        image.width(),
                        image.height(),
                        image.as_raw(),
                        texture.srgb,
                    );
                    self.textures.insert(key, gpu);
                    report.uploaded_textures += 1;
                    Some(key)
                }
                Err(error) => {
                    self.failed_textures.insert(key);
                    report.diagnostics.push(Scene3DRenderDiagnostic {
                        kind: Scene3DRenderDiagnosticKind::TextureDecodeFailed,
                        message: format!(
                            "failed to decode texture asset {} from '{}': {error}",
                            key.asset, descriptor.source
                        ),
                    });
                    None
                }
            },
            AssetBytes::Loading => {
                report.ready = false;
                report.diagnostics.push(Scene3DRenderDiagnostic {
                    kind: Scene3DRenderDiagnosticKind::TextureLoading,
                    message: format!(
                        "texture asset {} is loading from '{}'",
                        key.asset, descriptor.source
                    ),
                });
                None
            }
            AssetBytes::Failed(message) => {
                self.failed_textures.insert(key);
                report.diagnostics.push(Scene3DRenderDiagnostic {
                    kind: Scene3DRenderDiagnosticKind::TextureUnreadable,
                    message: format!(
                        "failed to read texture asset {} from '{}': {message}",
                        key.asset, descriptor.source
                    ),
                });
                None
            }
        }
    }

    fn ensure_pipeline(&mut self, device: &wgpu::Device, key: PipelineKey) {
        if self.pipelines.contains_key(&key) {
            return;
        }
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene3d pipeline layout"),
            bind_group_layouts: &[
                Some(&self.scene_layout),
                Some(&self.transform_layout),
                Some(&self.material_layout),
            ],
            immediate_size: 0,
        });
        let blend = match key.alpha {
            AlphaPipeline::Opaque => wgpu::BlendState::REPLACE,
            AlphaPipeline::Blend => wgpu::BlendState::ALPHA_BLENDING,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene3d material pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.target_format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: match key.cull {
                    CullMode3D::None => None,
                    CullMode3D::Front => Some(wgpu::Face::Front),
                    CullMode3D::Back => Some(wgpu::Face::Back),
                },
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(key.depth_write),
                depth_compare: Some(if key.depth_test {
                    wgpu::CompareFunction::Less
                } else {
                    wgpu::CompareFunction::Always
                }),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(key, pipeline);
    }

    fn clear_rect(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        viewport: Scene3DViewport,
        scissor: (u32, u32, u32, u32),
        color: [f32; 4],
    ) {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene3d clear color"),
            contents: bytemuck::cast_slice(&color),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene3d clear bind group"),
            layout: &self.clear_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scene3d clear encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene3d clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.clear_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
    }

    fn render_error_surface(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        viewport: Scene3DViewport,
    ) {
        if let Some((viewport, scissor)) = clamp_viewport(viewport, self.width, self.height) {
            self.clear_rect(
                device,
                queue,
                target,
                viewport,
                scissor,
                [0.24, 0.03, 0.08, 1.0],
            );
        }
    }
}

const DEFAULT_MATERIAL: Material3D = Material3D {
    revision: 0,
    asset: None,
    model: MaterialModel3D::MetallicRoughness {
        metallic: 0.0,
        roughness: 0.6,
    },
    base_color: fission_scene::Rgba::WHITE,
    base_color_texture: None,
    emissive_color: fission_scene::Rgba::TRANSPARENT,
    emissive_texture: None,
    alpha_mode: AlphaMode3D::Opaque,
    alpha_cutoff: 0.5,
    double_sided: false,
};

fn uniform_layout(
    device: &wgpu::Device,
    label: &str,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn depth_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene3d depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn upload_mesh(device: &wgpu::Device, mesh: &Mesh3D) -> GpuMesh {
    let vertices = mesh.vertices.iter().map(vertex).collect::<Vec<_>>();
    upload_raw_mesh(device, &vertices, &mesh.indices)
}

fn vertex(value: &MeshVertex3D) -> Vertex {
    Vertex {
        position: [value.position.x, value.position.y, value.position.z],
        normal: [value.normal.x, value.normal.y, value.normal.z],
        uv: [value.uv.x, value.uv.y],
    }
}

fn upload_raw_mesh(device: &wgpu::Device, vertices: &[Vertex], indices: &[u32]) -> GpuMesh {
    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scene3d retained vertex buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scene3d retained index buffer"),
        contents: bytemuck::cast_slice(indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    GpuMesh {
        vertex,
        index,
        index_count: indices.len() as u32,
    }
}

fn upload_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    srgb: bool,
) -> GpuTexture {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width.max(1) * 4),
            rows_per_image: Some(height.max(1)),
        },
        wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("scene3d nearest sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let linear = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("scene3d linear sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    GpuTexture {
        texture,
        view,
        nearest,
        linear,
    }
}

fn make_scene_uniforms(prepared: &PreparedScene3D, viewport: Scene3DViewport) -> SceneUniforms {
    let camera = prepared.source.camera;
    let view = Mat4::look_at_rh(
        Vec3::new(camera.eye.x, camera.eye.y, camera.eye.z),
        Vec3::new(camera.target.x, camera.target.y, camera.target.z),
        Vec3::new(camera.up.x, camera.up.y, camera.up.z).normalize(),
    );
    let aspect = (viewport.width / viewport.height).max(0.001);
    let projection = match camera.projection {
        CameraProjection3D::Perspective {
            vertical_fov_radians,
            near,
            far,
        } => Mat4::perspective_rh(vertical_fov_radians, aspect, near, far),
        CameraProjection3D::Orthographic {
            vertical_size,
            near,
            far,
        } => {
            let half_height = vertical_size * 0.5;
            let half_width = half_height * aspect;
            Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                near,
                far,
            )
        }
    };
    let mut result = SceneUniforms {
        view_projection: (projection * view).to_cols_array_2d(),
        camera_position: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
        ambient: [0.08, 0.08, 0.08, 1.0],
        directional_color: [0.0; 4],
        directional_direction: [0.0, -1.0, 0.0, 0.0],
        point_position_range: [[0.0; 4]; 4],
        point_color_intensity: [[0.0; 4]; 4],
        point_count: [0; 4],
    };
    let mut point = 0usize;
    for light in &prepared.source.lights {
        match *light {
            Light3D::Ambient(light) => {
                result.ambient = [light.color.r, light.color.g, light.color.b, light.intensity]
            }
            Light3D::Directional(light) => {
                result.directional_color =
                    [light.color.r, light.color.g, light.color.b, light.intensity];
                result.directional_direction =
                    [light.direction.x, light.direction.y, light.direction.z, 0.0];
            }
            Light3D::Point(light) if point < 4 => {
                result.point_position_range[point] = [
                    light.position.x,
                    light.position.y,
                    light.position.z,
                    light.range,
                ];
                result.point_color_intensity[point] =
                    [light.color.r, light.color.g, light.color.b, light.intensity];
                point += 1;
            }
            Light3D::Point(_) => {}
        }
    }
    result.point_count[0] = point as u32;
    result
}

fn transform_matrix(transform: fission_scene::Transform3) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::new(transform.scale.x, transform.scale.y, transform.scale.z),
        Quat::from_xyzw(
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
        ),
        Vec3::new(
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
        ),
    )
}

fn rgba(value: fission_scene::Rgba) -> [f32; 4] {
    [value.r, value.g, value.b, value.a]
}

fn cube_geometry() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let faces = [
        (
            [0.0, 0.0, 1.0],
            [
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5],
            ],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [0.5, -0.5, -0.5],
                [-0.5, -0.5, -0.5],
                [-0.5, 0.5, -0.5],
                [0.5, 0.5, -0.5],
            ],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [0.5, -0.5, 0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [0.5, 0.5, 0.5],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [-0.5, -0.5, -0.5],
                [-0.5, -0.5, 0.5],
                [-0.5, 0.5, 0.5],
                [-0.5, 0.5, -0.5],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [-0.5, 0.5, 0.5],
                [0.5, 0.5, 0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, -0.5, 0.5],
                [-0.5, -0.5, 0.5],
            ],
        ),
    ];
    for (normal, points) in faces {
        let base = vertices.len() as u32;
        for (position, uv) in
            points
                .into_iter()
                .zip([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])
        {
            vertices.push(Vertex {
                position,
                normal,
                uv,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

fn sphere_geometry(segments: u32, rings: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let phi = v * std::f32::consts::PI;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let theta = u * std::f32::consts::TAU;
            let normal = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            vertices.push(Vertex {
                position: normal,
                normal,
                uv: [u, v],
            });
        }
    }
    for ring in 0..rings {
        for segment in 0..segments {
            let first = ring * (segments + 1) + segment;
            let second = first + segments + 1;
            indices.extend_from_slice(&[first, second, first + 1, second, second + 1, first + 1]);
        }
    }
    (vertices, indices)
}

fn clamp_viewport(
    viewport: Scene3DViewport,
    width: u32,
    height: u32,
) -> Option<(Scene3DViewport, (u32, u32, u32, u32))> {
    if width == 0
        || height == 0
        || !viewport.x.is_finite()
        || !viewport.y.is_finite()
        || !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
    {
        return None;
    }
    let x0 = viewport.x.max(0.0).min(width as f32);
    let y0 = viewport.y.max(0.0).min(height as f32);
    let x1 = (viewport.x + viewport.width).max(0.0).min(width as f32);
    let y1 = (viewport.y + viewport.height).max(0.0).min(height as f32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let sx = x0.floor() as u32;
    let sy = y0.floor() as u32;
    let sw = (x1.ceil() as u32).min(width).saturating_sub(sx);
    let sh = (y1.ceil() as u32).min(height).saturating_sub(sy);
    Some((
        Scene3DViewport {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        },
        (sx, sy, sw, sh),
    ))
}

enum AssetBytes {
    Ready(Vec<u8>),
    Loading,
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
fn load_asset_bytes(source: &str) -> AssetBytes {
    let path = source
        .strip_prefix("asset://")
        .or_else(|| source.strip_prefix("file://"))
        .unwrap_or(source);
    let executable = std::env::current_exe().ok();
    let candidates = native_asset_candidates(std::path::Path::new(path), executable.as_deref());
    let mut last_error = None;
    for candidate in &candidates {
        match std::fs::read(candidate) {
            Ok(bytes) => return AssetBytes::Ready(bytes),
            Err(error) => last_error = Some(error),
        }
    }
    AssetBytes::Failed(format!(
        "could not read {source}; searched {}{}",
        candidates
            .iter()
            .map(|candidate| candidate.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        last_error
            .map(|error| format!(" ({error})"))
            .unwrap_or_default()
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn native_asset_candidates(
    path: &std::path::Path,
    executable: Option<&std::path::Path>,
) -> Vec<std::path::PathBuf> {
    if path.is_absolute() {
        return vec![path.to_owned()];
    }

    let mut candidates = vec![path.to_owned()];
    if let Some(executable_dir) = executable.and_then(std::path::Path::parent) {
        candidates.push(executable_dir.join(path));
        if let Some(application_root) = executable_dir.parent() {
            candidates.push(application_root.join(path));
            candidates.push(application_root.join("Resources").join(path));
        }
    }
    candidates.dedup();
    candidates
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WEB_ASSETS: std::cell::RefCell<HashMap<String, Option<Result<Vec<u8>, String>>>> = std::cell::RefCell::new(HashMap::new());
}

#[cfg(target_arch = "wasm32")]
fn load_asset_bytes(source: &str) -> AssetBytes {
    let source = source.strip_prefix("asset://").unwrap_or(source).to_owned();
    let state = WEB_ASSETS.with(|assets| assets.borrow().get(&source).cloned());
    match state {
        Some(Some(Ok(bytes))) => AssetBytes::Ready(bytes),
        Some(Some(Err(error))) => AssetBytes::Failed(error),
        Some(None) => AssetBytes::Loading,
        None => {
            WEB_ASSETS.with(|assets| {
                assets.borrow_mut().insert(source.clone(), None);
            });
            wasm_bindgen_futures::spawn_local(async move {
                let result = fetch_web_asset(&source).await;
                WEB_ASSETS.with(|assets| {
                    assets.borrow_mut().insert(source, Some(result));
                });
            });
            AssetBytes::Loading
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_web_asset(source: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().ok_or_else(|| "browser window unavailable".to_owned())?;
    let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(source))
        .await
        .map_err(|error| format!("fetch failed: {error:?}"))?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "fetch returned a non-response value".to_owned())?;
    if !response.ok() {
        return Err(format!("HTTP status {}", response.status()));
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(
        response
            .array_buffer()
            .map_err(|error| format!("array buffer unavailable: {error:?}"))?,
    )
    .await
    .map_err(|error| format!("reading response failed: {error:?}"))?;
    let bytes = js_sys::Uint8Array::new(&buffer);
    let mut output = vec![0; bytes.length() as usize];
    bytes.copy_to(&mut output);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_clamps_to_target() {
        let (_, scissor) = clamp_viewport(
            Scene3DViewport {
                x: -5.0,
                y: 2.0,
                width: 20.0,
                height: 20.0,
            },
            10,
            12,
        )
        .unwrap();
        assert_eq!(scissor, (0, 2, 10, 10));
    }

    #[test]
    fn generated_meshes_are_triangle_lists() {
        let (cube_vertices, cube_indices) = cube_geometry();
        let (sphere_vertices, sphere_indices) = sphere_geometry(8, 6);
        assert_eq!(cube_vertices.len(), 24);
        assert_eq!(cube_indices.len(), 36);
        assert!(!sphere_vertices.is_empty());
        assert_eq!(sphere_indices.len() % 3, 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_assets_resolve_from_run_and_packaged_layouts() {
        let candidates = native_asset_candidates(
            std::path::Path::new("assets/world.glb"),
            Some(std::path::Path::new("/bundle/Contents/MacOS/game")),
        );
        assert_eq!(candidates[0], std::path::Path::new("assets/world.glb"));
        assert!(candidates.contains(&std::path::PathBuf::from(
            "/bundle/Contents/MacOS/assets/world.glb"
        )));
        assert!(candidates.contains(&std::path::PathBuf::from(
            "/bundle/Contents/assets/world.glb"
        )));
        assert!(candidates.contains(&std::path::PathBuf::from(
            "/bundle/Contents/Resources/assets/world.glb"
        )));
    }
}
