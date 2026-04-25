use anyhow::Result;
use fission_core::diff::diff_ir;
use fission_core::env::{VideoStateMap, WebStateMap, Env};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{LayoutPoint, ScrollStateMap};
use fission_diagnostics::{SnapshotBlob, SnapshotKind, SnapshotProvider};
use fission_diagnostics::prelude as diag;
use fission_ir::{CoreIR, NodeId, Op, PaintOp, LayoutOp, EmbedKind, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutRect, LayoutSnapshot, LayoutUnit, LayoutSize};
use fission_render::{
    BoxShadow, Color as RenderColor, DisplayList, DisplayOp, Fill, ImageFit, Renderer, Stroke,
};
use fission_shell::VideoSurfaceFrame;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;

pub struct Pipeline {
    pub prev_ir: Option<CoreIR>, 
    pub last_snapshot: Option<LayoutSnapshot>,
    pub paint_cache: HashMap<NodeId, (u64, Vec<DisplayOp>)>,
    pub video_surfaces: Vec<VideoSurfaceFrame>,
    pub last_viewport: Option<LayoutRect>,
    pub layout_invariant_violation_count: u32,
    pub layout_full_rebuild_count: u32,
}

pub struct PipelineStats {
    pub dirty_nodes: usize,
    pub layout_updates: usize,
    pub paint_misses: usize,
    pub paint_hits: usize,
    pub video_surfaces: usize,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            prev_ir: None,
            last_snapshot: None,
            paint_cache: HashMap::new(),
            video_surfaces: Vec::new(),
            last_viewport: None,
            layout_invariant_violation_count: 0,
            layout_full_rebuild_count: 0,
        }
    }

    pub fn take_video_surfaces(&mut self) -> Vec<VideoSurfaceFrame> {
        std::mem::take(&mut self.video_surfaces)
    }

    pub fn render(
        &mut self,
        next_ir: CoreIR,
        viewport_size: LayoutSize,
        layout_engine: &mut LayoutEngine,
        scroll_map: &ScrollStateMap,
        renderer: &mut dyn Renderer,
        video_map: &VideoStateMap,
        web_map: &WebStateMap,
        env: &Env,
    ) -> Result<PipelineStats> {
        let viewport = LayoutRect::new(0.0, 0.0, viewport_size.width, viewport_size.height);
        let stats = self.update(next_ir, viewport, layout_engine, env, scroll_map, video_map, web_map)?;
        
        let snapshot = self.last_snapshot.take().expect("snapshot missing after update");
        let ir = self.prev_ir.take().expect("ir missing after update");
        
        let display_list = self.generate_display_list(&ir, &snapshot, scroll_map, video_map, web_map, viewport);
        
        self.last_snapshot = Some(snapshot);
        self.prev_ir = Some(ir);
        
        renderer.render(&display_list)?;
        Ok(stats)
    }

    fn generate_display_list(&mut self, ir: &CoreIR, snapshot: &LayoutSnapshot, scroll_map: &ScrollStateMap, video_map: &VideoStateMap, web_map: &WebStateMap, viewport: LayoutRect) -> DisplayList {
        let mut display_list = DisplayList {
            ops: Vec::new(),
            bounds: viewport,
        };

        let mut visited = HashSet::new();
        let mut flyout_contents = HashSet::new();
        for (id, node) in &ir.nodes {
            if let Op::Layout(LayoutOp::Flyout { content, .. }) = &node.op {
                flyout_contents.insert(*content);
            }
        }

        if let Some(root) = ir.root {
            self.generate_display_list_recursive_with_visited(
                root,
                ir,
                snapshot,
                scroll_map,
                &mut display_list,
                &mut 0, 
                &mut 0,
                video_map,
                web_map,
                LayoutPoint::ZERO,
                &mut visited,
                &flyout_contents,
            );
        }
        display_list
    }

    pub fn update(
        &mut self,
        next_ir: CoreIR,
        viewport: LayoutRect,
        layout_engine: &mut LayoutEngine,
        env: &Env,
        scroll_map: &ScrollStateMap,
        video_map: &VideoStateMap,
        web_map: &WebStateMap,
    ) -> Result<PipelineStats> {
        let mut stats = PipelineStats {
            dirty_nodes: 0,
            layout_updates: 0,
            paint_misses: 0,
            paint_hits: 0,
            video_surfaces: 0,
        };

        let viewport_changed = self.last_viewport.map(|v| v != viewport).unwrap_or(true);
        self.last_viewport = Some(viewport);

        let mut use_full = false;
        let mut layout_dirty_closure = HashSet::new();

        if let Some(last_ir) = &self.prev_ir {
            let diff = fission_core::diff::diff_ir(last_ir, &next_ir);
            layout_dirty_closure = diff.dirty_structural;
            stats.dirty_nodes = layout_dirty_closure.len();
        } else {
            use_full = true;
        }

        let needs_layout = !layout_dirty_closure.is_empty() || use_full || viewport_changed;

        // Always clear paint cache — it doesn't track child subtree changes
        // or layout position changes, so stale entries cause incorrect rendering
        // after resize, scroll, animation, or any child content change.
        self.paint_cache.clear();

        if needs_layout {
            let start_layout = std::time::Instant::now();
            let layout_input_nodes = build_layout_tree(&next_ir, env);
            let mut layout_dirty_count = layout_dirty_closure.len();

            if viewport_changed || use_full {
                let full: HashSet<NodeId> = layout_input_nodes.iter().map(|n| n.id).collect();
                layout_engine.update(&layout_input_nodes, &full);
                use_full = true;
                layout_dirty_count = full.len();
            } else {
                layout_engine.update(&layout_input_nodes, &layout_dirty_closure);
            }

            let root_id = next_ir.root.expect("no root in IR");
            let snapshot =
                layout_engine.compute_layout(&layout_input_nodes, root_id, viewport.size, &|id| {
                    scroll_map.get_offset(id)
                })?;
            self.last_snapshot = Some(snapshot);
            
            let duration = start_layout.elapsed().as_nanos() as u64;
            diag::emit(
                diag::DiagCategory::Layout,
                diag::DiagLevel::Debug,
                diag::DiagEventKind::LayoutSummary {
                    nodes: layout_input_nodes.len() as u32,
                    dirty_count: layout_dirty_count as u32,
                    full_rebuild: use_full,
                    duration_ns: duration,
                },
            );
        }

        let snapshot = self.last_snapshot.take().expect("layout snapshot missing");
        self.video_surfaces.clear();
        self.collect_video_surfaces(next_ir.root.expect("root missing"), &next_ir, &snapshot, video_map, scroll_map, LayoutPoint::ZERO);
        self.last_snapshot = Some(snapshot);

        self.prev_ir = Some(next_ir);
        stats.video_surfaces = self.video_surfaces.len();

        Ok(stats)
    }

    fn generate_display_list_recursive_with_visited(
        &mut self,
        node_id: NodeId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        scroll_map: &ScrollStateMap,
        out_list: &mut DisplayList,
        miss_count: &mut usize,
        hit_count: &mut usize,
        video_map: &VideoStateMap,
        web_map: &WebStateMap,
        accumulated_offset: LayoutPoint,
        visited: &mut HashSet<NodeId>,
        flyout_contents: &HashSet<NodeId>,
    ) {
        if !visited.insert(node_id) {
            return;
        }

        if let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            node.hash.hash(&mut hasher);
            
            // Simplified hashing for paint cache: include scroll offset if it's a scroll node
            if matches!(node.op, Op::Layout(LayoutOp::Scroll { .. })) {
                let offset = scroll_map.get_offset(node_id);
                offset.to_bits().hash(&mut hasher);
            }
            let hash = hasher.finish();

            if let Some((cached_hash, cached_ops)) = self.paint_cache.get(&node_id) {
                if *cached_hash == hash {
                    *hit_count += 1;
                    out_list.ops.extend(cached_ops.clone());
                    return;
                }
            }

            *miss_count += 1;
            let mut segment = Vec::new();
            let mut pushed_state = false;
            let mut child_offset = accumulated_offset;

            match &node.op {
                fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) => {
                    let offset = scroll_map.get_offset(node_id);
                    segment.push(DisplayOp::Save);
                    segment.push(DisplayOp::ClipRect(geom.rect));
                    match direction {
                        fission_ir::FlexDirection::Row => {
                            segment.push(DisplayOp::Translate(LayoutPoint::new(-offset, 0.0)));
                            child_offset =
                                LayoutPoint::new(accumulated_offset.x - offset, accumulated_offset.y);
                        }
                        fission_ir::FlexDirection::Column => {
                            segment.push(DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                            child_offset =
                                LayoutPoint::new(accumulated_offset.x, accumulated_offset.y - offset);
                        }
                    }
                    pushed_state = true;
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { path: _ }) => {
                    segment.push(DisplayOp::Save);
                    segment.push(DisplayOp::ClipRect(geom.rect));
                    pushed_state = true;
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Transform { transform }) => {
                    segment.push(DisplayOp::Save);
                    segment.push(DisplayOp::Transform(*transform));
                    pushed_state = true;
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect {
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                }) => {
                    segment.push(DisplayOp::DrawRect {
                        rect: geom.rect,
                        fill: fill.as_ref().map(|f| Fill {
                            color: RenderColor {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.as_ref().map(|s| Stroke {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        corner_radius: *corner_radius,
                        shadow: shadow.as_ref().map(|s| BoxShadow {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            blur_radius: s.blur_radius,
                            offset: s.offset,
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText {
                    text,
                    size,
                    color,
                    underline,
                    caret_index,
                }) => {
                    segment.push(DisplayOp::DrawText {
                        text: text.clone(),
                        position: geom.rect.origin,
                        size: *size,
                        color: fission_render::Color {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                        underline: *underline,
                        caret_index: *caret_index,
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText { runs, caret_index }) => {
                    let render_runs = runs
                        .iter()
                        .map(|r| fission_render::TextRun {
                            text: r.text.clone(),
                            style: fission_render::TextStyle {
                                font_size: r.style.font_size,
                                color: fission_render::Color {
                                    r: r.style.color.r,
                                    g: r.style.color.g,
                                    b: r.style.color.b,
                                    a: r.style.color.a,
                                },
                                underline: r.style.underline,
                                background_color: r.style.background_color.map(|c| fission_render::Color {
                                    r: c.r, g: c.g, b: c.b, a: c.a,
                                }),
                            },
                        })
                        .collect();

                    segment.push(DisplayOp::DrawRichText {
                        runs: render_runs,
                        position: geom.rect.origin,
                        bounds: geom.rect,
                        node_id: Some(node_id),
                        caret_index: *caret_index,
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawPath { path, fill, stroke }) => {
                    segment.push(DisplayOp::DrawPath {
                        path: path.clone(),
                        fill: fill.as_ref().map(|f| Fill {
                            color: RenderColor {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.as_ref().map(|s| Stroke {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawSvg { content, fill, stroke }) => {
                    segment.push(DisplayOp::DrawSvg {
                        content: content.clone(),
                        fill: fill.as_ref().map(|f| Fill {
                            color: RenderColor {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.as_ref().map(|s| Stroke {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                _ => {}
            }

            let mut temp_dl = DisplayList {
                ops: Vec::new(),
                bounds: out_list.bounds,
            };

            for child in &node.children {
                self.generate_display_list_recursive_with_visited(
                    *child,
                    ir,
                    snapshot,
                    scroll_map,
                    &mut temp_dl,
                    miss_count,
                    hit_count,
                    video_map,
                    web_map,
                    child_offset,
                    visited,
                    flyout_contents,
                );
            }

            segment.extend(temp_dl.ops);

            if pushed_state {
                segment.push(DisplayOp::Restore);
            }

            self.paint_cache.insert(node_id, (hash, segment.clone()));
            out_list.ops.extend(segment);
        }
    }

    fn push_video_surface(
        &mut self,
        widget_id: WidgetNodeId,
        rect: LayoutRect,
        video_map: &VideoStateMap,
    ) {
        if let Some(state) = video_map.states.get(&widget_id) {
            let surface_id = state.surface_id.unwrap_or(0);
            self.video_surfaces.push(VideoSurfaceFrame {
                widget_id,
                surface_id,
                rect,
            });
        }
    }

    fn collect_video_surfaces(
        &mut self,
        node_id: NodeId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        video_map: &VideoStateMap,
        scroll_map: &ScrollStateMap,
        accumulated_offset: LayoutPoint,
    ) {
        let mut visited = std::collections::HashSet::new();
        self.collect_video_surfaces_with_visited(
            node_id,
            ir,
            snapshot,
            video_map,
            scroll_map,
            accumulated_offset,
            &mut visited,
        );
    }

    fn collect_video_surfaces_with_visited(
        &mut self,
        node_id: NodeId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        video_map: &VideoStateMap,
        scroll_map: &ScrollStateMap,
        accumulated_offset: LayoutPoint,
        visited: &mut std::collections::HashSet<NodeId>,
    ) {
        if !visited.insert(node_id) {
            return;
        }
        if let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) {
            let mut child_offset = accumulated_offset;
            if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. }) = &node.op {
                let offset = scroll_map.get_offset(node_id);
                child_offset =
                    LayoutPoint::new(accumulated_offset.x, accumulated_offset.y - offset);
            }

            if let fission_ir::Op::Layout(fission_ir::LayoutOp::Embed {
                kind: EmbedKind::Video,
                widget_id,
                ..
            }) = &node.op
            {
                let translated_rect = translate_rect(geom.rect, accumulated_offset);
                self.push_video_surface(*widget_id, translated_rect, video_map);
            }

            for child in &node.children {
                self.collect_video_surfaces_with_visited(
                    *child,
                    ir,
                    snapshot,
                    video_map,
                    scroll_map,
                    child_offset,
                    visited,
                );
            }
        }
    }
}

impl SnapshotProvider for Pipeline {
    fn snapshot(&self, kind: SnapshotKind) -> Option<SnapshotBlob> {
        match kind {
            SnapshotKind::Layout => self.last_snapshot.as_ref().and_then(|snap| {
                serde_json::to_string_pretty(snap)
                    .ok()
                    .map(|json| SnapshotBlob { kind, json })
            }),
        }
    }
}

fn translate_rect(rect: LayoutRect, offset: LayoutPoint) -> LayoutRect {
    LayoutRect {
        origin: LayoutPoint::new(rect.origin.x + offset.x, rect.origin.y + offset.y),
        size: rect.size,
    }
}
