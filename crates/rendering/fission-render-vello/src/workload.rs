use fission_render::{DisplayList, DisplayOp, RenderLayer, RenderNode, RenderScene};
use vello::Scene;

pub fn workload_profile_for_scene(
    scene: &RenderScene,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
) -> vello::RenderWorkloadProfile {
    let mut builder = WorkloadProfileBuilder::new(width_px, height_px, scale_factor);
    for root in &scene.roots {
        builder.visit_node(root);
    }
    builder.finish()
}

/// Build a workload profile using the encoded Vello scene for text complexity.
///
/// Fission display operations retain complete text values so editing, selection,
/// and accessibility continue to work outside the visible viewport. The Vello
/// encoder culls those values to the glyphs that can actually contribute to the
/// frame. Using that encoded count prevents caller-sized GPU buffers from being
/// based on an entire large document while preserving Fission's more precise
/// coverage and composition context.
pub fn workload_profile_for_encoded_scene(
    scene: &RenderScene,
    encoded_scene: &Scene,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
) -> vello::RenderWorkloadProfile {
    let mut profile = workload_profile_for_scene(scene, width_px, height_px, scale_factor);
    let encoding = encoded_scene.encoding();
    profile.scene.glyphs = encoding.resources.glyphs.len().min(u32::MAX as usize) as u32;
    profile.scene.glyph_runs = encoding.resources.glyph_runs.len().min(u32::MAX as usize) as u32;
    profile
}

struct WorkloadProfileBuilder {
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    width_tiles: u32,
    height_tiles: u32,
    tile_ops: Vec<u16>,
    scene: vello::SceneComplexityProfile,
    total_draw_tile_coverage: u32,
    total_path_tile_coverage: u32,
    max_clip_depth: u32,
    max_blend_depth: u32,
    current_clip_depth: u32,
    current_blend_depth: u32,
}

impl WorkloadProfileBuilder {
    fn new(width_px: u32, height_px: u32, scale_factor: f64) -> Self {
        let width_tiles = width_px.next_multiple_of(16) / 16;
        let height_tiles = height_px.next_multiple_of(16) / 16;
        let tile_count = width_tiles.saturating_mul(height_tiles) as usize;
        Self {
            width_px,
            height_px,
            scale_factor,
            width_tiles,
            height_tiles,
            tile_ops: vec![0; tile_count],
            scene: vello::SceneComplexityProfile::default(),
            total_draw_tile_coverage: 0,
            total_path_tile_coverage: 0,
            max_clip_depth: 0,
            max_blend_depth: 0,
            current_clip_depth: 0,
            current_blend_depth: 0,
        }
    }

    fn finish(self) -> vello::RenderWorkloadProfile {
        let target_tiles = self.width_tiles.saturating_mul(self.height_tiles);
        let max_ops_per_tile = self.tile_ops.into_iter().max().unwrap_or(0) as u32;
        vello::RenderWorkloadProfile {
            target: vello::TargetProfile {
                width_px: self.width_px,
                height_px: self.height_px,
                scale_factor: self.scale_factor as f32,
                dirty_tiles: None,
            },
            coverage: vello::TileCoverageProfile {
                tile_width: 16,
                tile_height: 16,
                target_tiles,
                visible_tiles: target_tiles,
                total_draw_tile_coverage: self.total_draw_tile_coverage,
                total_path_tile_coverage: self.total_path_tile_coverage,
                max_ops_per_tile,
                max_blend_depth: self.max_blend_depth,
            },
            scene: vello::SceneComplexityProfile {
                max_clip_depth: self.max_clip_depth,
                ..self.scene
            },
            policy: vello::DynamicBufferPolicy {
                sizing: vello::BufferSizingMode::CallerEstimate,
                safety_margin_percent: 25,
                allow_grow_retry: true,
                max_dynamic_bytes: None,
            },
        }
    }

    fn visit_node(&mut self, node: &RenderNode) {
        match node {
            RenderNode::Paint(list) => self.visit_display_list(list),
            RenderNode::Layer(layer) => self.visit_layer(layer),
        }
    }

    fn visit_layer(&mut self, layer: &RenderLayer) {
        let clip_added = layer.style.clip.is_some();
        if clip_added {
            self.scene.clip_ops = self.scene.clip_ops.saturating_add(1);
            self.current_clip_depth = self.current_clip_depth.saturating_add(1);
            self.max_clip_depth = self.max_clip_depth.max(self.current_clip_depth);
        }
        let blend_added = (layer.style.opacity - 1.0).abs() > 0.001;
        if blend_added {
            self.scene.blend_ops = self.scene.blend_ops.saturating_add(1);
            self.current_blend_depth = self.current_blend_depth.saturating_add(1);
            self.max_blend_depth = self.max_blend_depth.max(self.current_blend_depth);
            self.add_coverage(layer.bounds, false);
        }
        for child in &layer.children {
            self.visit_node(child);
        }
        if blend_added {
            self.current_blend_depth = self.current_blend_depth.saturating_sub(1);
        }
        if clip_added {
            self.current_clip_depth = self.current_clip_depth.saturating_sub(1);
        }
    }

    fn visit_display_list(&mut self, list: &DisplayList) {
        for op in &list.ops {
            match op {
                DisplayOp::Save
                | DisplayOp::Restore
                | DisplayOp::Translate(_)
                | DisplayOp::Transform(_) => {}
                DisplayOp::ClipRect(_) | DisplayOp::ClipRoundedRect { .. } => {
                    self.scene.clip_ops = self.scene.clip_ops.saturating_add(1);
                    self.max_clip_depth = self.max_clip_depth.max(self.current_clip_depth + 1);
                }
                DisplayOp::OpacityLayer { bounds, .. } => {
                    self.scene.blend_ops = self.scene.blend_ops.saturating_add(1);
                    self.max_blend_depth = self.max_blend_depth.max(self.current_blend_depth + 1);
                    self.add_coverage(*bounds, false);
                }
                DisplayOp::CachedScene { list, bounds, .. } => {
                    self.add_coverage(*bounds, false);
                    self.visit_display_list(list);
                }
                DisplayOp::BackdropFilter { bounds, .. } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.scene.blend_ops = self.scene.blend_ops.saturating_add(1);
                    self.add_coverage(*bounds, false);
                }
                DisplayOp::DrawRect {
                    bounds,
                    stroke,
                    corner_radius,
                    shadow,
                    ..
                } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    if stroke.is_some() || *corner_radius > 0.0 {
                        self.scene.path_ops = self.scene.path_ops.saturating_add(1);
                        self.scene.estimated_path_segments =
                            self.scene.estimated_path_segments.saturating_add(8);
                        self.add_coverage(*bounds, true);
                    } else {
                        self.add_coverage(*bounds, false);
                    }
                    if shadow.is_some() {
                        self.scene.blend_ops = self.scene.blend_ops.saturating_add(1);
                    }
                }
                DisplayOp::DrawText { text, bounds, .. } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.scene.glyph_runs = self.scene.glyph_runs.saturating_add(1);
                    let glyphs = text.chars().count() as u32;
                    self.scene.glyphs = self.scene.glyphs.saturating_add(glyphs);
                    self.scene.path_ops = self.scene.path_ops.saturating_add(1);
                    self.add_coverage(*bounds, true);
                }
                DisplayOp::DrawRichText { runs, bounds, .. } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.scene.glyph_runs = self.scene.glyph_runs.saturating_add(runs.len() as u32);
                    let glyphs = runs
                        .iter()
                        .map(|run| run.text.chars().count() as u32)
                        .fold(0_u32, u32::saturating_add);
                    self.scene.glyphs = self.scene.glyphs.saturating_add(glyphs);
                    self.scene.path_ops = self.scene.path_ops.saturating_add(1);
                    self.add_coverage(*bounds, true);
                }
                DisplayOp::DrawImage { bounds, .. } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.scene.images = self.scene.images.saturating_add(1);
                    self.add_coverage(*bounds, false);
                }
                DisplayOp::DrawPath { path, bounds, .. } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.scene.path_ops = self.scene.path_ops.saturating_add(1);
                    self.scene.path_points = self
                        .scene
                        .path_points
                        .saturating_add(estimate_svg_path_points(path));
                    self.add_coverage(*bounds, true);
                }
                DisplayOp::DrawSvg {
                    content, bounds, ..
                } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.scene.path_ops = self
                        .scene
                        .path_ops
                        .saturating_add(estimate_svg_shape_count(content));
                    self.scene.path_points = self
                        .scene
                        .path_points
                        .saturating_add((content.len() as u32 / 8).max(1));
                    self.add_coverage(*bounds, true);
                }
                DisplayOp::DrawSurface { bounds, .. } => {
                    self.scene.draw_ops = self.scene.draw_ops.saturating_add(1);
                    self.add_coverage(*bounds, false);
                }
            }
        }
    }

    fn add_coverage(&mut self, rect: fission_render::LayoutRect, path_like: bool) {
        let Some((x0, y0, x1, y1)) = self.tile_range(rect) else {
            return;
        };
        let covered = x1.saturating_sub(x0).saturating_mul(y1.saturating_sub(y0));
        self.total_draw_tile_coverage = self.total_draw_tile_coverage.saturating_add(covered);
        if path_like {
            self.total_path_tile_coverage = self.total_path_tile_coverage.saturating_add(covered);
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = y.saturating_mul(self.width_tiles).saturating_add(x) as usize;
                if let Some(count) = self.tile_ops.get_mut(idx) {
                    *count = count.saturating_add(1);
                }
            }
        }
    }

    fn tile_range(&self, rect: fission_render::LayoutRect) -> Option<(u32, u32, u32, u32)> {
        if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            return None;
        }
        let scale = self.scale_factor.max(0.0001);
        let x0 =
            ((rect.origin.x as f64 * scale).floor().max(0.0) as u32 / 16).min(self.width_tiles);
        let y0 =
            ((rect.origin.y as f64 * scale).floor().max(0.0) as u32 / 16).min(self.height_tiles);
        let x1_px = ((rect.origin.x + rect.size.width) as f64 * scale)
            .ceil()
            .clamp(0.0, self.width_px as f64) as u32;
        let y1_px = ((rect.origin.y + rect.size.height) as f64 * scale)
            .ceil()
            .clamp(0.0, self.height_px as f64) as u32;
        let x1 = x1_px.div_ceil(16).min(self.width_tiles);
        let y1 = y1_px.div_ceil(16).min(self.height_tiles);
        if x1 <= x0 || y1 <= y0 {
            None
        } else {
            Some((x0, y0, x1, y1))
        }
    }
}

fn estimate_svg_path_points(path: &str) -> u32 {
    let commands = path
        .bytes()
        .filter(|b| {
            matches!(
                b,
                b'M' | b'm'
                    | b'L'
                    | b'l'
                    | b'H'
                    | b'h'
                    | b'V'
                    | b'v'
                    | b'C'
                    | b'c'
                    | b'S'
                    | b's'
                    | b'Q'
                    | b'q'
                    | b'T'
                    | b't'
                    | b'A'
                    | b'a'
                    | b'Z'
                    | b'z'
            )
        })
        .count() as u32;
    commands.max(path.len() as u32 / 24).max(1)
}

fn estimate_svg_shape_count(content: &str) -> u32 {
    let paths = content.matches("<path").count() as u32;
    let rects = content.matches("<rect").count() as u32;
    let circles = content.matches("<circle").count() as u32;
    paths.saturating_add(rects).saturating_add(circles).max(1)
}
