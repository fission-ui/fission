use crate::pipeline::CompositorTexturePlan;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use fission_layout::LayoutPoint;
use fission_render::LayerClip;
use fission_render::Renderer as _;
use fission_render_vello::{RetainedSceneCache, VelloRenderer, VelloTextMeasurer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use vello::wgpu;
use vello::{RenderParams, Renderer as VelloSceneRenderer, Scene};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
struct LayerUniform {
    rect: [f32; 4],
    clip: [f32; 4],
    clip_local: [f32; 4],
    clip_shape: [f32; 4],
    viewport_and_opacity: [f32; 4],
    transform: [[f32; 4]; 4],
}

struct CachedLayerTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    content_key: u64,
    base: Option<CachedBaseTexture>,
    last_uniform: Option<LayerUniform>,
    last_used_frame: u64,
}

struct CachedBaseTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    scene_cache_key: Option<u64>,
}

struct DrawBatch {
    layer_key: u64,
    clip: (u32, u32, u32, u32),
}

#[derive(Clone)]
struct RetainedCompositorLayer {
    has_texture: bool,
    bounds: fission_layout::LayoutRect,
    clip: Option<LayerClip>,
    opacity: f32,
    transform: Option<[f32; 16]>,
    transform_clip: bool,
    scene_cache_key: Option<u64>,
    content_key: u64,
    local_dynamic: bool,
    composite_dynamic: bool,
    children: Vec<u64>,
    local_dirty: bool,
    structure_dirty: bool,
    composite_dirty: bool,
    last_draw_rect: Option<fission_layout::LayoutRect>,
    last_used_frame: u64,
}

impl RetainedCompositorLayer {
    fn new(plan: &CompositorTexturePlan, frame: u64) -> Self {
        Self {
            has_texture: plan.scene.is_some(),
            bounds: plan.bounds,
            clip: plan.clip.clone(),
            opacity: plan.opacity,
            transform: plan.transform,
            transform_clip: plan.transform_clip,
            scene_cache_key: plan.scene_cache_key,
            content_key: plan.content_key,
            local_dynamic: plan.local_dynamic,
            composite_dynamic: plan.composite_dynamic,
            children: plan.children.iter().map(|child| child.key).collect(),
            local_dirty: true,
            structure_dirty: true,
            composite_dirty: true,
            last_draw_rect: None,
            last_used_frame: frame,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct CompositorFrameStats {
    pub damage_rect: Option<fission_layout::LayoutRect>,
    pub rendered_layers: usize,
    pub partial_recomposites: usize,
    pub full_recomposites: usize,
    pub reused_base_textures: usize,
    pub evicted_layers: usize,
    pub resident_texture_bytes: usize,
}

#[derive(Clone, Copy)]
struct RenderOutcome {
    changed: bool,
    damage_rect: Option<fission_layout::LayoutRect>,
}

#[derive(Clone, Copy)]
struct TargetContext {
    origin: LayoutPoint,
    inherited_transform: Option<[f32; 16]>,
    inherited_scissor: Option<(u32, u32, u32, u32)>,
    viewport_width: u32,
    viewport_height: u32,
}

pub struct TextureLayerCompositor {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<u64, CachedLayerTexture>,
    layers: HashMap<u64, RetainedCompositorLayer>,
    root_layers: Vec<u64>,
    frame_index: u64,
    texture_budget_bytes: usize,
    last_target_size: Option<(u32, u32)>,
}

impl TextureLayerCompositor {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fission-compositor shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compositor.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fission-compositor bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fission-compositor pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fission-compositor pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fission-compositor sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            textures: HashMap::new(),
            layers: HashMap::new(),
            root_layers: Vec::new(),
            frame_index: 0,
            texture_budget_bytes: compositor_cache_budget_bytes(),
            last_target_size: None,
        }
    }

    pub fn prune(
        &mut self,
        live_keys: &HashSet<u64>,
    ) -> (Option<fission_layout::LayoutRect>, bool) {
        let mut removed_damage = None;
        let mut removed_any = false;

        self.textures.retain(|key, _| live_keys.contains(key));
        self.layers.retain(|key, layer| {
            if live_keys.contains(key) {
                true
            } else {
                removed_any = true;
                removed_damage = union_layout_rects(removed_damage, layer.last_draw_rect);
                false
            }
        });
        self.root_layers.retain(|key| live_keys.contains(key));
        (removed_damage, removed_any)
    }

    pub fn render_layers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vello_renderer: &mut VelloSceneRenderer,
        retained_scene_cache: &mut RetainedSceneCache,
        measurer: Arc<VelloTextMeasurer>,
        scale_factor: f64,
        viewport_width: u32,
        viewport_height: u32,
        root_transform: Option<[f32; 16]>,
        plans: &[CompositorTexturePlan],
        force_full_redraw: bool,
        clear_color: wgpu::Color,
        target_view: &wgpu::TextureView,
    ) -> Result<CompositorFrameStats> {
        self.frame_index = self.frame_index.saturating_add(1);
        let live_keys = self.sync_plans(plans);
        let (pruned_damage, removed_layers) = self.prune(&live_keys);

        let mut stats = CompositorFrameStats::default();
        let root_ctx = TargetContext {
            origin: LayoutPoint::ZERO,
            inherited_transform: root_transform,
            inherited_scissor: None,
            viewport_width,
            viewport_height,
        };
        for plan in plans {
            let outcome = self.render_plan_layer(
                device,
                queue,
                vello_renderer,
                retained_scene_cache,
                Arc::clone(&measurer),
                scale_factor,
                plan,
                root_ctx,
                &mut stats,
            )?;
            stats.damage_rect = union_layout_rects(stats.damage_rect, outcome.damage_rect);
        }
        stats.damage_rect = union_layout_rects(stats.damage_rect, pruned_damage);

        let target_size_changed = self.last_target_size != Some((viewport_width, viewport_height));
        let full_target_redraw = force_full_redraw
            || target_size_changed
            || removed_layers
            || stats.damage_rect.is_none();
        let damage_scissor = if full_target_redraw {
            None
        } else {
            stats.damage_rect.map(|rect| {
                logical_rect_to_physical_scissor(
                    rect,
                    scale_factor,
                    viewport_width,
                    viewport_height,
                )
            })
        };
        if full_target_redraw || damage_scissor.is_some() {
            let root_layers = self.root_layers.clone();
            let draw_batches = self.build_layer_draw_batches(
                queue,
                device,
                scale_factor,
                viewport_width,
                viewport_height,
                LayoutPoint::ZERO,
                root_transform,
                1.0,
                None,
                &root_layers,
                damage_scissor,
            );
            self.draw_batches_to_view(
                device,
                queue,
                target_view,
                draw_batches,
                if full_target_redraw {
                    wgpu::LoadOp::Clear(clear_color)
                } else {
                    wgpu::LoadOp::Load
                },
            );
        }
        self.last_target_size = Some((viewport_width, viewport_height));

        stats.evicted_layers = self.enforce_texture_budget();
        stats.resident_texture_bytes = self.resident_texture_bytes();
        Ok(stats)
    }

    fn sync_plans(&mut self, plans: &[CompositorTexturePlan]) -> HashSet<u64> {
        let mut live = HashSet::new();
        self.root_layers.clear();
        for plan in plans {
            self.sync_plan(plan, &mut live);
            self.root_layers.push(plan.key);
        }
        live
    }

    fn sync_plan(&mut self, plan: &CompositorTexturePlan, live: &mut HashSet<u64>) {
        live.insert(plan.key);
        for child in &plan.children {
            self.sync_plan(child, live);
        }

        let next_children: Vec<u64> = plan.children.iter().map(|child| child.key).collect();
        match self.layers.get_mut(&plan.key) {
            Some(layer) => {
                layer.last_used_frame = self.frame_index;
                let had_texture = layer.has_texture;
                let texture_changed = had_texture != plan.scene.is_some();
                let size_changed = layer.bounds.size != plan.bounds.size;
                let scene_changed = layer.scene_cache_key != plan.scene_cache_key;
                let structure_changed = layer.children != next_children;
                let style_changed = layer.clip != plan.clip
                    || (layer.opacity - plan.opacity).abs() > 0.001
                    || layer.transform != plan.transform
                    || layer.transform_clip != plan.transform_clip;
                layer.has_texture = plan.scene.is_some();
                layer.bounds = plan.bounds;
                layer.clip = plan.clip.clone();
                layer.opacity = plan.opacity;
                layer.transform = plan.transform;
                layer.transform_clip = plan.transform_clip;
                layer.scene_cache_key = plan.scene_cache_key;
                layer.content_key = plan.content_key;
                layer.local_dynamic = plan.local_dynamic;
                layer.composite_dynamic = plan.composite_dynamic;
                layer.children = next_children;
                layer.local_dirty |=
                    texture_changed || size_changed || scene_changed || plan.local_dynamic;
                layer.structure_dirty |= structure_changed;
                layer.composite_dirty |= style_changed || structure_changed;
            }
            None => {
                self.layers.insert(
                    plan.key,
                    RetainedCompositorLayer::new(plan, self.frame_index),
                );
            }
        }
    }

    fn render_plan_layer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vello_renderer: &mut VelloSceneRenderer,
        retained_scene_cache: &mut RetainedSceneCache,
        measurer: Arc<VelloTextMeasurer>,
        scale_factor: f64,
        plan: &CompositorTexturePlan,
        target_ctx: TargetContext,
        stats: &mut CompositorFrameStats,
    ) -> Result<RenderOutcome> {
        let state = self
            .layers
            .get(&plan.key)
            .cloned()
            .expect("missing retained compositor layer state");
        let localized = localize_world_transform(plan.transform, target_ctx.origin);
        let combined = combine_transforms(target_ctx.inherited_transform, localized);
        let scoped_scissor = intersect_scissor(
            target_ctx.inherited_scissor,
            plan.clip.as_ref(),
            if state.transform_clip {
                combined
            } else {
                target_ctx.inherited_transform
            },
            target_ctx.origin,
            scale_factor,
            target_ctx.viewport_width,
            target_ctx.viewport_height,
        );
        let current_draw_rect = logical_draw_rect_for_plan(
            plan,
            target_ctx.origin,
            if state.transform_clip {
                combined
            } else {
                target_ctx.inherited_transform
            },
            scale_factor,
            target_ctx.viewport_width,
            target_ctx.viewport_height,
        );

        if plan.scene.is_none() {
            let child_ctx = TargetContext {
                origin: target_ctx.origin,
                inherited_transform: combined,
                inherited_scissor: Some(scoped_scissor),
                viewport_width: target_ctx.viewport_width,
                viewport_height: target_ctx.viewport_height,
            };
            let mut child_damage = None;
            let mut child_changed = false;
            for child in &plan.children {
                let outcome = self.render_plan_layer(
                    device,
                    queue,
                    vello_renderer,
                    retained_scene_cache,
                    Arc::clone(&measurer),
                    scale_factor,
                    child,
                    child_ctx,
                    stats,
                )?;
                child_changed |= outcome.changed;
                child_damage = union_layout_rects(child_damage, outcome.damage_rect);
            }

            let wrapper_damage =
                if state.composite_dirty || state.structure_dirty || state.composite_dynamic {
                    union_layout_rects(state.last_draw_rect, current_draw_rect)
                } else {
                    child_damage
                };
            if let Some(layer) = self.layers.get_mut(&plan.key) {
                layer.last_draw_rect = current_draw_rect;
                layer.local_dirty = false;
                layer.structure_dirty = false;
                layer.composite_dirty = false;
            }
            return Ok(RenderOutcome {
                changed: child_changed || wrapper_damage.is_some(),
                damage_rect: wrapper_damage,
            });
        }

        let texture_width = logical_to_physical(plan.bounds.size.width, scale_factor);
        let texture_height = logical_to_physical(plan.bounds.size.height, scale_factor);
        let local_ctx = TargetContext {
            origin: plan.bounds.origin,
            inherited_transform: None,
            inherited_scissor: None,
            viewport_width: texture_width,
            viewport_height: texture_height,
        };

        let mut child_damage_local = None;
        let mut child_changed = false;
        for child in &plan.children {
            let outcome = self.render_plan_layer(
                device,
                queue,
                vello_renderer,
                retained_scene_cache,
                Arc::clone(&measurer),
                scale_factor,
                child,
                local_ctx,
                stats,
            )?;
            child_changed |= outcome.changed;
            child_damage_local = union_layout_rects(child_damage_local, outcome.damage_rect);
        }

        let mut created = false;
        {
            let cached = self.textures.entry(plan.key).or_insert_with(|| {
                created = true;
                create_cached_texture(
                    device,
                    &self.bind_group_layout,
                    &self.sampler,
                    texture_width,
                    texture_height,
                )
            });
            if cached.width != texture_width || cached.height != texture_height {
                *cached = create_cached_texture(
                    device,
                    &self.bind_group_layout,
                    &self.sampler,
                    texture_width,
                    texture_height,
                );
                created = true;
            }
            cached.last_used_frame = self.frame_index;
        }

        let full_recompose = created
            || state.local_dirty
            || state.structure_dirty
            || state.local_dynamic
            || child_damage_local.is_none() && child_changed;
        let can_partial_recompose = !full_recompose && child_damage_local.is_some();
        let needs_recompose = full_recompose || can_partial_recompose;
        if needs_recompose {
            stats.rendered_layers += 1;

            if full_recompose {
                render_or_seed_layer_base(
                    self.textures
                        .get_mut(&plan.key)
                        .expect("missing cached compositor layer"),
                    device,
                    queue,
                    vello_renderer,
                    retained_scene_cache,
                    Arc::clone(&measurer),
                    scale_factor,
                    plan.scene.as_ref(),
                    plan.scene_cache_key.filter(|_| !plan.local_dynamic),
                    texture_width,
                    texture_height,
                    None,
                )?;
                stats.full_recomposites += 1;
            } else if let Some(dirty_rect) = child_damage_local {
                let dirty_scissor = logical_rect_to_physical_scissor(
                    dirty_rect,
                    scale_factor,
                    texture_width,
                    texture_height,
                );
                render_or_seed_layer_base(
                    self.textures
                        .get_mut(&plan.key)
                        .expect("missing cached compositor layer"),
                    device,
                    queue,
                    vello_renderer,
                    retained_scene_cache,
                    Arc::clone(&measurer),
                    scale_factor,
                    plan.scene.as_ref(),
                    plan.scene_cache_key.filter(|_| !plan.local_dynamic),
                    texture_width,
                    texture_height,
                    Some(dirty_scissor),
                )?;
                stats.partial_recomposites += 1;
                stats.reused_base_textures += 1;
            }

            if !plan.children.is_empty() {
                let local_clip = if can_partial_recompose {
                    child_damage_local.map(|rect| {
                        logical_rect_to_physical_scissor(
                            rect,
                            scale_factor,
                            texture_width,
                            texture_height,
                        )
                    })
                } else {
                    None
                };
                let draw_batches = self.build_layer_draw_batches(
                    queue,
                    device,
                    scale_factor,
                    texture_width,
                    texture_height,
                    plan.bounds.origin,
                    None,
                    1.0,
                    local_clip,
                    &state.children,
                    None,
                );
                let target_view = &self
                    .textures
                    .get(&plan.key)
                    .expect("missing cached compositor layer")
                    .view;
                self.draw_batches_to_view(
                    device,
                    queue,
                    target_view,
                    draw_batches,
                    wgpu::LoadOp::Load,
                );
            }
            if let Some(cached) = self.textures.get_mut(&plan.key) {
                cached.content_key = plan.content_key;
            }
        }

        let parent_damage = if needs_recompose || state.composite_dirty || state.composite_dynamic {
            union_layout_rects(state.last_draw_rect, current_draw_rect)
        } else {
            None
        };
        if let Some(layer) = self.layers.get_mut(&plan.key) {
            layer.last_draw_rect = current_draw_rect;
            layer.local_dirty = false;
            layer.structure_dirty = false;
            layer.composite_dirty = false;
            layer.last_used_frame = self.frame_index;
        }

        Ok(RenderOutcome {
            changed: needs_recompose || state.composite_dirty || state.composite_dynamic,
            damage_rect: parent_damage,
        })
    }

    fn build_layer_draw_batches(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        scale_factor: f64,
        viewport_width: u32,
        viewport_height: u32,
        target_origin: LayoutPoint,
        inherited_transform: Option<[f32; 16]>,
        inherited_opacity: f32,
        inherited_scissor: Option<(u32, u32, u32, u32)>,
        layer_keys: &[u64],
        extra_scissor: Option<(u32, u32, u32, u32)>,
    ) -> Vec<DrawBatch> {
        let mut batches = Vec::with_capacity(layer_keys.len());
        for key in layer_keys {
            let Some(layer) = self.layers.get(key).cloned() else {
                continue;
            };
            if let (Some(extra), Some(last_draw_rect)) = (extra_scissor, layer.last_draw_rect) {
                let draw_scissor = logical_rect_to_physical_scissor(
                    last_draw_rect,
                    scale_factor,
                    viewport_width,
                    viewport_height,
                );
                if intersect_scissor_optional(Some(draw_scissor), Some(extra)).is_none() {
                    continue;
                }
            }
            let localized = localize_world_transform(layer.transform, target_origin);
            let combined = combine_transforms(inherited_transform, localized);
            let clip_transform = if layer.transform_clip {
                combined
            } else {
                inherited_transform
            };
            let clip = intersect_scissor(
                inherited_scissor,
                layer.clip.as_ref(),
                clip_transform,
                target_origin,
                scale_factor,
                viewport_width,
                viewport_height,
            );
            let Some(clip) = intersect_scissor_optional(Some(clip), extra_scissor) else {
                continue;
            };
            if clip.2 == 0 || clip.3 == 0 || clip.0 >= viewport_width || clip.1 >= viewport_height {
                continue;
            }
            let opacity = inherited_opacity * layer.opacity;

            if !layer.has_texture {
                batches.extend(self.build_layer_draw_batches(
                    queue,
                    device,
                    scale_factor,
                    viewport_width,
                    viewport_height,
                    target_origin,
                    combined,
                    opacity,
                    Some(clip),
                    &layer.children,
                    extra_scissor,
                ));
                continue;
            }

            let (clip_local, clip_shape) =
                local_clip_mask(layer.clip.as_ref(), layer.bounds, scale_factor);
            let Some(cached) = self.textures.get_mut(key) else {
                continue;
            };
            let rect = [
                ((layer.bounds.origin.x - target_origin.x) as f64 * scale_factor) as f32,
                ((layer.bounds.origin.y - target_origin.y) as f64 * scale_factor) as f32,
                cached.width as f32,
                cached.height as f32,
            ];
            let uniform = LayerUniform {
                rect,
                clip: [clip.0 as f32, clip.1 as f32, clip.2 as f32, clip.3 as f32],
                clip_local,
                clip_shape,
                viewport_and_opacity: [viewport_width as f32, viewport_height as f32, opacity, 0.0],
                transform: matrix_to_rows(scale_transform(combined, scale_factor)),
            };
            if cached.last_uniform != Some(uniform) {
                queue.write_buffer(&cached.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
                cached.last_uniform = Some(uniform);
            }
            batches.push(DrawBatch {
                layer_key: *key,
                clip,
            });
        }
        batches
    }

    fn draw_batches_to_view(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        draw_batches: Vec<DrawBatch>,
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fission-compositor encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fission-compositor pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            for batch in &draw_batches {
                pass.set_scissor_rect(
                    batch.clip.0,
                    batch.clip.1,
                    batch.clip.2.max(1),
                    batch.clip.3.max(1),
                );
                let Some(cached) = self.textures.get(&batch.layer_key) else {
                    continue;
                };
                pass.set_bind_group(0, &cached.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
    }

    fn resident_texture_bytes(&self) -> usize {
        self.textures
            .values()
            .map(|cached| {
                let main = texture_bytes(cached.width, cached.height);
                let base = cached
                    .base
                    .as_ref()
                    .map(|base| texture_bytes(base.width, base.height))
                    .unwrap_or(0);
                main + base
            })
            .sum()
    }

    fn enforce_texture_budget(&mut self) -> usize {
        let mut current = self.resident_texture_bytes();
        if current <= self.texture_budget_bytes {
            return 0;
        }

        let mut victims: Vec<(u64, u64)> = self
            .textures
            .iter()
            .map(|(key, cached)| (*key, cached.last_used_frame))
            .collect();
        victims.sort_by_key(|(_, last_used)| *last_used);

        let mut evicted = 0;
        for (key, _) in victims {
            if current <= self.texture_budget_bytes {
                break;
            }
            if let Some(cached) = self.textures.remove(&key) {
                current = current.saturating_sub(texture_bytes(cached.width, cached.height));
                if let Some(base) = cached.base {
                    current = current.saturating_sub(texture_bytes(base.width, base.height));
                }
                evicted += 1;
            }
        }
        evicted
    }
}

fn union_layout_rects(
    lhs: Option<fission_layout::LayoutRect>,
    rhs: Option<fission_layout::LayoutRect>,
) -> Option<fission_layout::LayoutRect> {
    match (lhs, rhs) {
        (Some(a), Some(b)) => Some(rect_union(a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn logical_to_physical(value: f32, scale_factor: f64) -> u32 {
    ((value as f64 * scale_factor).round() as u32).max(1)
}

fn texture_bytes(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

fn compositor_cache_budget_bytes() -> usize {
    std::env::var("FISSION_COMPOSITOR_CACHE_MB")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(256)
        .saturating_mul(1024 * 1024)
}

fn matrix_to_rows(matrix: [f32; 16]) -> [[f32; 4]; 4] {
    [
        [matrix[0], matrix[1], matrix[2], matrix[3]],
        [matrix[4], matrix[5], matrix[6], matrix[7]],
        [matrix[8], matrix[9], matrix[10], matrix[11]],
        [matrix[12], matrix[13], matrix[14], matrix[15]],
    ]
}

fn scale_transform(transform: Option<[f32; 16]>, scale_factor: f64) -> [f32; 16] {
    let mut matrix = transform.unwrap_or(identity_matrix());
    matrix[12] *= scale_factor as f32;
    matrix[13] *= scale_factor as f32;
    matrix
}

fn localize_world_transform(
    transform: Option<[f32; 16]>,
    target_origin: LayoutPoint,
) -> Option<[f32; 16]> {
    let transform = transform?;
    let to_local = translation_matrix(-target_origin.x, -target_origin.y);
    let from_local = translation_matrix(target_origin.x, target_origin.y);
    Some(multiply_matrix(
        to_local,
        multiply_matrix(transform, from_local),
    ))
}

fn combine_transforms(inherited: Option<[f32; 16]>, local: Option<[f32; 16]>) -> Option<[f32; 16]> {
    match (inherited, local) {
        (Some(lhs), Some(rhs)) => Some(multiply_matrix(lhs, rhs)),
        (Some(lhs), None) => Some(lhs),
        (None, Some(rhs)) => Some(rhs),
        (None, None) => None,
    }
}

fn intersect_scissor(
    inherited_scissor: Option<(u32, u32, u32, u32)>,
    clip: Option<&LayerClip>,
    transform: Option<[f32; 16]>,
    target_origin: LayoutPoint,
    scale_factor: f64,
    viewport_width: u32,
    viewport_height: u32,
) -> (u32, u32, u32, u32) {
    let base = clip
        .map(|_| {
            clip_rect_to_physical(
                clip,
                transform,
                target_origin,
                scale_factor,
                viewport_width,
                viewport_height,
            )
        })
        .unwrap_or((0, 0, viewport_width.max(1), viewport_height.max(1)));
    match inherited_scissor {
        Some(parent) => intersect_scissor_rects(parent, base),
        None => base,
    }
}

fn clip_rect_to_physical(
    clip: Option<&LayerClip>,
    transform: Option<[f32; 16]>,
    target_origin: LayoutPoint,
    scale_factor: f64,
    viewport_width: u32,
    viewport_height: u32,
) -> (u32, u32, u32, u32) {
    let Some(rect) = clip_rect(clip) else {
        return (0, 0, viewport_width.max(1), viewport_height.max(1));
    };

    let mut points = [
        LayoutPoint::new(
            rect.origin.x - target_origin.x,
            rect.origin.y - target_origin.y,
        ),
        LayoutPoint::new(
            rect.origin.x + rect.size.width - target_origin.x,
            rect.origin.y - target_origin.y,
        ),
        LayoutPoint::new(
            rect.origin.x - target_origin.x,
            rect.origin.y + rect.size.height - target_origin.y,
        ),
        LayoutPoint::new(
            rect.origin.x + rect.size.width - target_origin.x,
            rect.origin.y + rect.size.height - target_origin.y,
        ),
    ];
    if let Some(matrix) = transform {
        for point in &mut points {
            *point = transform_point(matrix, *point);
        }
    }

    let min_x = points
        .iter()
        .map(|p| p.x)
        .fold(f32::INFINITY, f32::min)
        .max(0.0);
    let min_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min)
        .max(0.0);
    let max_x = points
        .iter()
        .map(|p| p.x)
        .fold(f32::NEG_INFINITY, f32::max)
        .min(viewport_width as f32 / scale_factor as f32);
    let max_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max)
        .min(viewport_height as f32 / scale_factor as f32);

    let x = (min_x as f64 * scale_factor).round().max(0.0) as u32;
    let y = (min_y as f64 * scale_factor).round().max(0.0) as u32;
    let max_x = (max_x as f64 * scale_factor)
        .round()
        .clamp(0.0, viewport_width as f64) as u32;
    let max_y = (max_y as f64 * scale_factor)
        .round()
        .clamp(0.0, viewport_height as f64) as u32;
    if x >= viewport_width || y >= viewport_height || max_x <= x || max_y <= y {
        return (viewport_width, viewport_height, 0, 0);
    }
    (
        x.min(viewport_width),
        y.min(viewport_height),
        max_x.saturating_sub(x),
        max_y.saturating_sub(y),
    )
}

fn intersect_scissor_rects(
    lhs: (u32, u32, u32, u32),
    rhs: (u32, u32, u32, u32),
) -> (u32, u32, u32, u32) {
    let left = lhs.0.max(rhs.0);
    let top = lhs.1.max(rhs.1);
    let right = lhs.0.saturating_add(lhs.2).min(rhs.0.saturating_add(rhs.2));
    let bottom = lhs.1.saturating_add(lhs.3).min(rhs.1.saturating_add(rhs.3));
    (
        left,
        top,
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
}

fn intersect_scissor_optional(
    lhs: Option<(u32, u32, u32, u32)>,
    rhs: Option<(u32, u32, u32, u32)>,
) -> Option<(u32, u32, u32, u32)> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => {
            let left = lhs.0.max(rhs.0);
            let top = lhs.1.max(rhs.1);
            let right = lhs.0.saturating_add(lhs.2).min(rhs.0.saturating_add(rhs.2));
            let bottom = lhs.1.saturating_add(lhs.3).min(rhs.1.saturating_add(rhs.3));
            let width = right.saturating_sub(left);
            let height = bottom.saturating_sub(top);
            if width == 0 || height == 0 {
                None
            } else {
                Some((left, top, width, height))
            }
        }
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}

fn clip_rect(clip: Option<&LayerClip>) -> Option<fission_layout::LayoutRect> {
    match clip {
        Some(LayerClip::Rect(rect)) => Some(*rect),
        Some(LayerClip::RoundedRect { rect, .. }) => Some(*rect),
        None => None,
    }
}

fn transform_point(matrix: [f32; 16], point: LayoutPoint) -> LayoutPoint {
    LayoutPoint::new(
        matrix[0] * point.x + matrix[4] * point.y + matrix[12],
        matrix[1] * point.x + matrix[5] * point.y + matrix[13],
    )
}

fn logical_draw_rect_for_plan(
    plan: &CompositorTexturePlan,
    target_origin: LayoutPoint,
    transform: Option<[f32; 16]>,
    scale_factor: f64,
    viewport_width: u32,
    viewport_height: u32,
) -> Option<fission_layout::LayoutRect> {
    let rect = clip_rect(plan.clip.as_ref()).unwrap_or(plan.bounds);
    let scissor = clip_rect_to_physical(
        Some(&LayerClip::Rect(rect)),
        transform,
        target_origin,
        scale_factor,
        viewport_width,
        viewport_height,
    );
    physical_scissor_to_logical_rect(scissor, scale_factor)
}

fn physical_scissor_to_logical_rect(
    scissor: (u32, u32, u32, u32),
    scale_factor: f64,
) -> Option<fission_layout::LayoutRect> {
    if scissor.2 == 0 || scissor.3 == 0 {
        return None;
    }
    let scale = scale_factor as f32;
    Some(fission_layout::LayoutRect::new(
        scissor.0 as f32 / scale,
        scissor.1 as f32 / scale,
        scissor.2 as f32 / scale,
        scissor.3 as f32 / scale,
    ))
}

fn rect_union(
    lhs: fission_layout::LayoutRect,
    rhs: fission_layout::LayoutRect,
) -> fission_layout::LayoutRect {
    let left = lhs.origin.x.min(rhs.origin.x);
    let top = lhs.origin.y.min(rhs.origin.y);
    let right = (lhs.origin.x + lhs.size.width).max(rhs.origin.x + rhs.size.width);
    let bottom = (lhs.origin.y + lhs.size.height).max(rhs.origin.y + rhs.size.height);
    fission_layout::LayoutRect::new(left, top, right - left, bottom - top)
}

fn logical_rect_to_physical_scissor(
    rect: fission_layout::LayoutRect,
    scale_factor: f64,
    viewport_width: u32,
    viewport_height: u32,
) -> (u32, u32, u32, u32) {
    let x = ((rect.origin.x as f64) * scale_factor).round().max(0.0) as u32;
    let y = ((rect.origin.y as f64) * scale_factor).round().max(0.0) as u32;
    if x >= viewport_width || y >= viewport_height {
        return (viewport_width, viewport_height, 0, 0);
    }
    let w = ((rect.size.width as f64) * scale_factor).round().max(0.0) as u32;
    let h = ((rect.size.height as f64) * scale_factor).round().max(0.0) as u32;
    (
        x.min(viewport_width),
        y.min(viewport_height),
        w.min(viewport_width.saturating_sub(x)),
        h.min(viewport_height.saturating_sub(y)),
    )
}

fn local_clip_mask(
    clip: Option<&LayerClip>,
    bounds: fission_layout::LayoutRect,
    scale_factor: f64,
) -> ([f32; 4], [f32; 4]) {
    match clip {
        Some(LayerClip::Rect(rect)) => (
            [
                ((rect.origin.x - bounds.origin.x) as f64 * scale_factor) as f32,
                ((rect.origin.y - bounds.origin.y) as f64 * scale_factor) as f32,
                (rect.size.width as f64 * scale_factor) as f32,
                (rect.size.height as f64 * scale_factor) as f32,
            ],
            [1.0, 0.0, 0.0, 0.0],
        ),
        Some(LayerClip::RoundedRect { rect, radius }) => (
            [
                ((rect.origin.x - bounds.origin.x) as f64 * scale_factor) as f32,
                ((rect.origin.y - bounds.origin.y) as f64 * scale_factor) as f32,
                (rect.size.width as f64 * scale_factor) as f32,
                (rect.size.height as f64 * scale_factor) as f32,
            ],
            [2.0, (*radius as f64 * scale_factor) as f32, 0.0, 0.0],
        ),
        None => (
            [
                0.0,
                0.0,
                (bounds.size.width as f64 * scale_factor) as f32,
                (bounds.size.height as f64 * scale_factor) as f32,
            ],
            [0.0, 0.0, 0.0, 0.0],
        ),
    }
}

fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn translation_matrix(tx: f32, ty: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
    ]
}

fn multiply_matrix(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

fn create_cached_texture(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
) -> CachedLayerTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fission-compositor layer texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fission-compositor uniform"),
        size: std::mem::size_of::<LayerUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("fission-compositor bind group"),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
        ],
    });
    CachedLayerTexture {
        texture,
        view,
        uniform_buffer,
        bind_group,
        width,
        height,
        content_key: 0,
        base: None,
        last_uniform: None,
        last_used_frame: 0,
    }
}

fn create_base_texture(device: &wgpu::Device, width: u32, height: u32) -> CachedBaseTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fission-compositor base texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    CachedBaseTexture {
        texture,
        view,
        width,
        height,
        scene_cache_key: None,
    }
}

fn ensure_base_texture(
    cached: &mut CachedLayerTexture,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    vello_renderer: &mut VelloSceneRenderer,
    retained_scene_cache: &mut RetainedSceneCache,
    measurer: Arc<VelloTextMeasurer>,
    scale_factor: f64,
    scene: Option<&fission_render::RenderScene>,
    scene_cache_key: Option<u64>,
    width: u32,
    height: u32,
) -> Result<()> {
    let needs_render = {
        let base = cached
            .base
            .get_or_insert_with(|| create_base_texture(device, width, height));
        if base.width != width || base.height != height {
            *base = create_base_texture(device, width, height);
        }
        base.scene_cache_key != scene_cache_key
    };
    if needs_render {
        let base = cached
            .base
            .as_mut()
            .expect("missing base texture after creation");
        if let Some(scene) = scene {
            render_plan_scene(
                device,
                queue,
                vello_renderer,
                retained_scene_cache,
                measurer,
                scale_factor,
                scene,
                scene_cache_key,
                &base.view,
                width,
                height,
            )?;
        } else {
            clear_target_view(device, queue, &base.view, wgpu::Color::TRANSPARENT);
        }
        base.scene_cache_key = scene_cache_key;
    }
    Ok(())
}

fn copy_texture_to_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src: &wgpu::Texture,
    dst: &wgpu::Texture,
    width: u32,
    height: u32,
    region: Option<(u32, u32, u32, u32)>,
) {
    let (origin_x, origin_y, copy_width, copy_height) = region.unwrap_or((0, 0, width, height));
    if copy_width == 0 || copy_height == 0 {
        return;
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("fission-compositor copy encoder"),
    });
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: src,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: origin_x,
                y: origin_y,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: dst,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: origin_x,
                y: origin_y,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: copy_width.max(1),
            height: copy_height.max(1),
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
}

fn render_or_seed_layer_base(
    cached: &mut CachedLayerTexture,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    vello_renderer: &mut VelloSceneRenderer,
    retained_scene_cache: &mut RetainedSceneCache,
    measurer: Arc<VelloTextMeasurer>,
    scale_factor: f64,
    scene: Option<&fission_render::RenderScene>,
    scene_cache_key: Option<u64>,
    width: u32,
    height: u32,
    region: Option<(u32, u32, u32, u32)>,
) -> Result<()> {
    ensure_base_texture(
        cached,
        device,
        queue,
        vello_renderer,
        retained_scene_cache,
        measurer,
        scale_factor,
        scene,
        scene_cache_key,
        width,
        height,
    )?;
    if let Some(base) = &cached.base {
        copy_texture_to_texture(
            device,
            queue,
            &base.texture,
            &cached.texture,
            width,
            height,
            region,
        );
    }
    Ok(())
}

fn clear_target_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target_view: &wgpu::TextureView,
    color: wgpu::Color,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("fission-compositor clear encoder"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fission-compositor clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    queue.submit(Some(encoder.finish()));
}

fn render_plan_scene(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    vello_renderer: &mut VelloSceneRenderer,
    retained_scene_cache: &mut RetainedSceneCache,
    measurer: Arc<VelloTextMeasurer>,
    scale_factor: f64,
    scene: &fission_render::RenderScene,
    scene_cache_key: Option<u64>,
    target_view: &wgpu::TextureView,
    width: u32,
    height: u32,
) -> Result<()> {
    let params = RenderParams {
        base_color: vello::peniko::Color::from_rgba8(0, 0, 0, 0),
        width,
        height,
        antialiasing_method: vello::AaConfig::Area,
    };

    if let Some(cache_key) = scene_cache_key {
        let cached_scene = retained_scene_cache.get_or_insert_with(cache_key, |scene_cache| {
            let mut encoded = Scene::new();
            let mut renderer = VelloRenderer::new(
                &mut encoded,
                Arc::clone(&measurer),
                scene_cache,
                scale_factor,
            );
            renderer.render_scene(scene)?;
            Ok(encoded)
        })?;
        vello_renderer.render_to_texture(device, queue, cached_scene, target_view, &params)?;
        return Ok(());
    }

    let mut encoded = Scene::new();
    let mut renderer =
        VelloRenderer::new(&mut encoded, measurer, retained_scene_cache, scale_factor);
    renderer.render_scene(scene)?;
    vello_renderer.render_to_texture(device, queue, &encoded, target_view, &params)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_layout::LayoutRect;

    fn plan_with_clip(
        clip: Option<LayerClip>,
        transform: Option<[f32; 16]>,
    ) -> CompositorTexturePlan {
        CompositorTexturePlan {
            key: 1,
            bounds: LayoutRect::new(10.0, 20.0, 120.0, 80.0),
            scene: None,
            scene_cache_key: None,
            content_key: 1,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform,
            transform_clip: true,
            clip,
            children: Vec::new(),
            source_layer_path: None,
        }
    }

    #[test]
    fn rounded_clip_mask_preserves_radius() {
        let (clip_local, clip_shape) = local_clip_mask(
            Some(&LayerClip::RoundedRect {
                rect: LayoutRect::new(14.0, 26.0, 64.0, 32.0),
                radius: 8.0,
            }),
            LayoutRect::new(10.0, 20.0, 120.0, 80.0),
            2.0,
        );
        assert_eq!(clip_shape[0], 2.0);
        assert_eq!(clip_shape[1], 16.0);
        assert_eq!(clip_local, [8.0, 12.0, 128.0, 64.0]);
    }

    #[test]
    fn draw_rect_respects_transform() {
        let plan = plan_with_clip(
            Some(LayerClip::Rect(LayoutRect::new(10.0, 20.0, 40.0, 20.0))),
            Some(translation_matrix(12.0, 6.0)),
        );
        let rect =
            logical_draw_rect_for_plan(&plan, LayoutPoint::ZERO, plan.transform, 1.0, 400, 300)
                .expect("draw rect");
        assert_eq!(rect.origin.x, 22.0);
        assert_eq!(rect.origin.y, 26.0);
        assert_eq!(rect.size.width, 40.0);
        assert_eq!(rect.size.height, 20.0);
    }

    #[test]
    fn disjoint_damage_scissor_skips_batch() {
        let clip = intersect_scissor_optional(Some((0, 0, 20, 20)), Some((40, 40, 10, 10)));
        assert!(clip.is_none());
    }
}
