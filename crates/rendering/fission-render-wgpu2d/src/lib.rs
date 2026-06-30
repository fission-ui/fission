use bytemuck::{Pod, Zeroable};
use fission_render::{
    surface_placeholder_color, Color, DisplayList, DisplayOp, Fill, LayerClip, LayoutRect,
    RenderLayer, RenderNode, RenderScene, Stroke,
};
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct Uniforms {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let clip_x = (input.position.x / uniforms.viewport.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (input.position.y / uniforms.viewport.y) * 2.0;
    out.position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

const PLACEHOLDER_COLOR: Color = Color {
    r: 255,
    g: 0,
    b: 180,
    a: 96,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
pub struct Wgpu2dRenderParams {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub clear_color: Color,
}

impl Wgpu2dRenderParams {
    pub fn new(width: u32, height: u32, scale_factor: f32, clear_color: Color) -> Self {
        Self {
            width,
            height,
            scale_factor,
            clear_color,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Wgpu2dRenderStats {
    pub rect_quads: u32,
    pub placeholder_quads: u32,
    pub unsupported_ops: u32,
}

pub struct Wgpu2dRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Wgpu2dRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fission-wgpu2d shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fission-wgpu2d bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<Uniforms>() as wgpu::BufferAddress
                    ),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fission-wgpu2d pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fission-wgpu2d pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fission-wgpu2d uniforms"),
            size: std::mem::size_of::<Uniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fission-wgpu2d bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &RenderScene,
        target_view: &wgpu::TextureView,
        params: Wgpu2dRenderParams,
    ) -> anyhow::Result<Wgpu2dRenderStats> {
        let width = params.width.max(1);
        let height = params.height.max(1);
        let scale_factor = if params.scale_factor.is_finite() && params.scale_factor > 0.0 {
            params.scale_factor
        } else {
            1.0
        };
        let uniforms = Uniforms {
            viewport: [width as f32, height as f32],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut builder = QuadBuilder::new(scale_factor);
        builder.emit_scene(scene);

        let vertex_buffer = (!builder.vertices.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("fission-wgpu2d vertices"),
                contents: bytemuck::cast_slice(&builder.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fission-wgpu2d encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fission-wgpu2d pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu_color(params.clear_color)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if let Some(vertex_buffer) = &vertex_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..builder.vertices.len() as u32, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));

        Ok(builder.stats)
    }
}

struct QuadBuilder {
    scale_factor: f32,
    vertices: Vec<Vertex>,
    stats: Wgpu2dRenderStats,
}

impl QuadBuilder {
    fn new(scale_factor: f32) -> Self {
        Self {
            scale_factor,
            vertices: Vec::new(),
            stats: Wgpu2dRenderStats::default(),
        }
    }

    fn emit_scene(&mut self, scene: &RenderScene) {
        for node in &scene.roots {
            self.emit_node(node);
        }
    }

    fn emit_node(&mut self, node: &RenderNode) {
        match node {
            RenderNode::Paint(list) => self.emit_display_list(list),
            RenderNode::Layer(layer) => self.emit_layer(layer),
        }
    }

    fn emit_layer(&mut self, layer: &RenderLayer) {
        if layer.style.clip.is_some() || layer.style.transform.is_some() {
            self.stats.unsupported_ops = self.stats.unsupported_ops.saturating_add(1);
            if let Some(clip) = &layer.style.clip {
                match clip {
                    LayerClip::Rect(rect) => self.emit_placeholder(*rect),
                    LayerClip::RoundedRect { rect, .. } => self.emit_placeholder(*rect),
                }
            }
        }
        for child in &layer.children {
            self.emit_node(child);
        }
    }

    fn emit_display_list(&mut self, list: &DisplayList) {
        for op in &list.ops {
            self.emit_op(op);
        }
    }

    fn emit_op(&mut self, op: &DisplayOp) {
        match op {
            DisplayOp::DrawRect {
                rect, fill, stroke, ..
            } => self.emit_rect(*rect, fill.as_ref(), stroke.as_ref()),
            DisplayOp::CachedScene { list, .. } => self.emit_display_list(list),
            DisplayOp::DrawSurface {
                rect,
                surface_id,
                position,
                ..
            } => {
                self.emit_quad(*rect, surface_placeholder_color(*surface_id, *position));
                self.stats.placeholder_quads = self.stats.placeholder_quads.saturating_add(1);
                self.stats.unsupported_ops = self.stats.unsupported_ops.saturating_add(1);
            }
            DisplayOp::DrawText { bounds, .. }
            | DisplayOp::DrawRichText { bounds, .. }
            | DisplayOp::DrawImage { bounds, .. }
            | DisplayOp::DrawPath { bounds, .. }
            | DisplayOp::DrawSvg { bounds, .. } => self.emit_placeholder(*bounds),
            DisplayOp::Save
            | DisplayOp::Restore
            | DisplayOp::ClipRect(_)
            | DisplayOp::ClipRoundedRect { .. }
            | DisplayOp::OpacityLayer { .. }
            | DisplayOp::Translate(_)
            | DisplayOp::Transform(_) => {
                self.stats.unsupported_ops = self.stats.unsupported_ops.saturating_add(1);
            }
        }
    }

    fn emit_rect(&mut self, rect: LayoutRect, fill: Option<&Fill>, stroke: Option<&Stroke>) {
        let mut emitted = false;
        match fill {
            Some(Fill::Solid(color)) => {
                self.emit_quad(rect, *color);
                self.stats.rect_quads = self.stats.rect_quads.saturating_add(1);
                emitted = true;
            }
            Some(Fill::LinearGradient { .. } | Fill::RadialGradient { .. }) => {
                self.stats.unsupported_ops = self.stats.unsupported_ops.saturating_add(1);
            }
            None => {}
        }

        if let Some(stroke) = stroke {
            if let Fill::Solid(color) = &stroke.fill {
                self.emit_stroke(rect, stroke.width, *color);
                emitted = true;
            } else {
                self.stats.unsupported_ops = self.stats.unsupported_ops.saturating_add(1);
            }
        }

        if !emitted {
            self.emit_placeholder(rect);
        }
    }

    fn emit_stroke(&mut self, rect: LayoutRect, width: f32, color: Color) {
        let width = width
            .max(0.0)
            .min(rect.size.width / 2.0)
            .min(rect.size.height / 2.0);
        if width <= 0.0 {
            return;
        }
        let x = rect.origin.x;
        let y = rect.origin.y;
        let w = rect.size.width;
        let h = rect.size.height;
        self.emit_quad(LayoutRect::new(x, y, w, width), color);
        self.emit_quad(LayoutRect::new(x, y + h - width, w, width), color);
        self.emit_quad(LayoutRect::new(x, y + width, width, h - width * 2.0), color);
        self.emit_quad(
            LayoutRect::new(x + w - width, y + width, width, h - width * 2.0),
            color,
        );
        self.stats.rect_quads = self.stats.rect_quads.saturating_add(4);
    }

    fn emit_placeholder(&mut self, rect: LayoutRect) {
        self.emit_quad(rect, PLACEHOLDER_COLOR);
        self.stats.placeholder_quads = self.stats.placeholder_quads.saturating_add(1);
        self.stats.unsupported_ops = self.stats.unsupported_ops.saturating_add(1);
    }

    fn emit_quad(&mut self, rect: LayoutRect, color: Color) {
        if color.a == 0 {
            return;
        }
        let x0 = rect.origin.x * self.scale_factor;
        let y0 = rect.origin.y * self.scale_factor;
        let x1 = (rect.origin.x + rect.size.width) * self.scale_factor;
        let y1 = (rect.origin.y + rect.size.height) * self.scale_factor;
        if !x0.is_finite()
            || !y0.is_finite()
            || !x1.is_finite()
            || !y1.is_finite()
            || x1 <= x0
            || y1 <= y0
        {
            return;
        }
        let color = normalized_color(color);
        self.vertices.extend_from_slice(&[
            Vertex {
                position: [x0, y0],
                color,
            },
            Vertex {
                position: [x1, y0],
                color,
            },
            Vertex {
                position: [x0, y1],
                color,
            },
            Vertex {
                position: [x0, y1],
                color,
            },
            Vertex {
                position: [x1, y0],
                color,
            },
            Vertex {
                position: [x1, y1],
                color,
            },
        ]);
    }
}

fn normalized_color(color: Color) -> [f32; 4] {
    [
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    ]
}

fn wgpu_color(color: Color) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(color.r) / 255.0,
        g: f64::from(color.g) / 255.0,
        b: f64::from(color.b) / 255.0,
        a: f64::from(color.a) / 255.0,
    }
}
