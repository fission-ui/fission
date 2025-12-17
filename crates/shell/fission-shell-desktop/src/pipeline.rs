use anyhow::Result;
use fission_diagnostics::prelude as diag;
use fission_diagnostics::{SnapshotProvider, SnapshotKind, SnapshotBlob};
use std::fs::File;
use std::io::Write as _;
use fission_core::diff::diff_ir;
use fission_core::env::VideoStateMap;
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{LayoutPoint, ScrollStateMap};
use fission_ir::op::EmbedKind;
use fission_ir::{CoreIR, NodeId, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutRect, LayoutSize, LayoutSnapshot};
use fission_render::{
    BoxShadow, Color as RenderColor, DisplayList, DisplayOp, Fill, ImageFit, Renderer, Stroke,
};
use fission_shell::VideoSurfaceFrame;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct Pipeline {
    pub prev_ir: Option<CoreIR>,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub paint_cache: HashMap<NodeId, (u64, Vec<DisplayOp>)>,
    video_surfaces: Vec<VideoSurfaceFrame>,
    // instrumentation
    pub layout_full_rebuild_count: usize,
    pub layout_cycle_detected_count: usize,
    pub layout_invariant_violation_count: usize,
}

#[derive(Debug, Default)]
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
            layout_full_rebuild_count: 0,
            layout_cycle_detected_count: 0,
            layout_invariant_violation_count: 0,
        }
    }

    pub fn render<'r>(
        &'r mut self,
        next_ir: CoreIR,
        viewport: LayoutSize,
        layout_engine: &mut LayoutEngine,
        scroll_map: &ScrollStateMap,
        renderer: &mut (impl Renderer + 'r + ?Sized),
        video_map: &VideoStateMap,
    ) -> Result<PipelineStats> {
        if let Some(cycle) = detect_ir_cycle(&next_ir) {
            diag::emit(
                diag::DiagCategory::Invariants,
                diag::DiagLevel::Error,
                diag::DiagEventKind::InvariantViolation {
                    kind: "ir_cycle".into(),
                    node: cycle.first().map(|n| n.as_u128()),
                    details: format!("cycle_len={} first={:?}", cycle.len(), &cycle[..cycle.len().min(6)]),
                    dump_ref: None,
                },
            );
            // Avoid crashing the frame; render nothing this frame.
            return Ok(PipelineStats {
                dirty_nodes: 0,
                layout_updates: 0,
                paint_misses: 0,
                paint_hits: 0,
                video_surfaces: 0,
            });
        }
        let dirty_set = if let Some(prev) = &self.prev_ir {
            let diff = diff_ir(prev, &next_ir);
            // println!("Diff: {} dirty nodes", diff.dirty_structural.len());
            // Debug: highlight structural children changes for a small subset
            let mut logged = 0usize;
            for id in &diff.dirty_structural {
                if let (Some(pn), Some(nn)) = (prev.nodes.get(id), next_ir.nodes.get(id)) {
                    if pn.children != nn.children {
                        logged += 1;
                        if logged <= 20 {
                            diag::emit(
                                diag::DiagCategory::Diff,
                                diag::DiagLevel::Debug,
                                diag::DiagEventKind::DiffSummary {
                                    nodes_total: next_ir.nodes.len() as u32,
                                    nodes_created: 0,
                                    nodes_removed: 0,
                                    nodes_changed: 1,
                                    dirty_layout: diff.dirty_structural.len() as u32,
                                    dirty_paint: 0,
                                },
                            );
                        }
                    }
                } else if prev.nodes.get(id).is_none() && next_ir.nodes.get(id).is_some() {
                    logged += 1;
                    if logged <= 20 {
                        diag::emit(
                            diag::DiagCategory::Diff,
                            diag::DiagLevel::Debug,
                            diag::DiagEventKind::DiffSummary {
                                nodes_total: next_ir.nodes.len() as u32,
                                nodes_created: 1,
                                nodes_removed: 0,
                                nodes_changed: 0,
                                dirty_layout: diff.dirty_structural.len() as u32,
                                dirty_paint: 0,
                            },
                        );
                    }
                }
            }
            diff.dirty_structural
        } else {
            // println!("Diff: Full Rebuild");
            next_ir.nodes.keys().cloned().collect()
        };

        // Expand dirty set to include ancestors (safety for parent-child edge updates)
        let mut dirty_with_ancestors = dirty_set.clone();
        for id in &dirty_set {
            let mut cur = next_ir.nodes.get(id).and_then(|n| n.parent);
            while let Some(p) = cur {
                if !dirty_with_ancestors.insert(p) { break; }
                cur = next_ir.nodes.get(&p).and_then(|n| n.parent);
            }
        }

        // Also include descendants of dirty nodes to ensure child parentage is fully refreshed
        let mut dirty_closure = dirty_with_ancestors.clone();
        let mut stack: Vec<_> = dirty_with_ancestors.iter().cloned().collect();
        while let Some(id) = stack.pop() {
            if let Some(n) = next_ir.nodes.get(&id) {
                for &child in &n.children {
                    if dirty_closure.insert(child) {
                        stack.push(child);
                    }
                }
            }
        }

        // Heuristic: if the change set is large, force full rebuild
        let dirty_count = dirty_closure.len();
        let total_nodes = next_ir.nodes.len();
        let use_full = dirty_count * 2 > total_nodes;

        let layout_input_nodes = build_layout_tree(&next_ir);
        diag::emit(
            diag::DiagCategory::Layout,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::LayoutSummary {
                nodes: layout_input_nodes.len() as u32,
                dirty_count: dirty_count as u32,
                full_rebuild: use_full,
            },
        );
        // Invariant validation (fatal in debug/strict)
        if let Some(root) = next_ir.root {
            if let Err(e) = validate_layout_invariants(&layout_input_nodes, root) {
                // Try to dump a snapshot for diagnostics
                let dump_ref = if let Some(snap) = &self.last_snapshot {
                    let path = std::env::temp_dir().join("fission_layout_snapshot.json");
                    if let Ok(mut f) = File::create(&path) {
                        if let Ok(json) = serde_json::to_string_pretty(snap) {
                            let bytes = json.into_bytes();
                            let _ = f.write_all(&bytes);
                            Some(path.display().to_string())
                        } else { None }
                    } else { None }
                } else { None };
                diag::emit(
                    diag::DiagCategory::Invariants,
                    diag::DiagLevel::Error,
                    diag::DiagEventKind::InvariantViolation {
                        kind: "pre_update".into(),
                        node: None,
                        details: format!("{}", e),
                        dump_ref,
                    },
                );
                self.layout_invariant_violation_count += 1;
                let strict = std::env::var("FISSION_LAYOUT_STRICT").ok().as_deref() == Some("1");
                if cfg!(debug_assertions) || strict {
                    panic!("layout invariant violation: {}", e);
                } else {
                    // optional diagnostic rebuild
                    let allow_rebuild = std::env::var("FISSION_ALLOW_FULL_REBUILD").ok().as_deref() == Some("1");
                    if allow_rebuild {
                        diag::emit(
                            diag::DiagCategory::Layout,
                            diag::DiagLevel::Warn,
                            diag::DiagEventKind::LayoutSummary {
                                nodes: layout_input_nodes.len() as u32,
                                dirty_count: dirty_count as u32,
                                full_rebuild: true,
                            },
                        );
                        layout_engine.rebuild(&layout_input_nodes)?;
                        self.layout_full_rebuild_count += 1;
                    } else {
                        return Ok(PipelineStats { dirty_nodes: 0, layout_updates: 0, paint_misses: 0, paint_hits: 0, video_surfaces: 0 });
                    }
                }
            }
        }

        if use_full {
            let full: std::collections::HashSet<_> = layout_input_nodes.iter().map(|n| n.id).collect();
            layout_engine.update(&layout_input_nodes, &full);
        } else {
            layout_engine.update(&layout_input_nodes, &dirty_closure);
        }
        // End of layout update; nothing to emit here (summary above)

        // Post-update verification
        if let Some(root) = next_ir.root {
            if let Err(e) = layout_engine.verify_post_update(&layout_input_nodes, root) {
                // Try to dump a snapshot for diagnostics
                let dump_ref = if let Some(snap) = &self.last_snapshot {
                    let path = std::env::temp_dir().join("fission_layout_snapshot.json");
                    if let Ok(mut f) = File::create(&path) {
                        if let Ok(json) = serde_json::to_string_pretty(snap) {
                            let bytes = json.into_bytes();
                            let _ = f.write_all(&bytes);
                            Some(path.display().to_string())
                        } else { None }
                    } else { None }
                } else { None };
                diag::emit(
                    diag::DiagCategory::Invariants,
                    diag::DiagLevel::Error,
                    diag::DiagEventKind::InvariantViolation { kind: "post_update".into(), node: None, details: format!("{}", e), dump_ref },
                );
                self.layout_invariant_violation_count += 1;
                let strict = std::env::var("FISSION_LAYOUT_STRICT").ok().as_deref() == Some("1");
                if cfg!(debug_assertions) || strict {
                    panic!("layout post-update verification failed: {}", e);
                } else {
                    let allow_rebuild = std::env::var("FISSION_ALLOW_FULL_REBUILD").ok().as_deref() == Some("1");
                    if allow_rebuild {
                        diag::emit(
                            diag::DiagCategory::Layout,
                            diag::DiagLevel::Warn,
                            diag::DiagEventKind::LayoutSummary { nodes: layout_input_nodes.len() as u32, dirty_count: dirty_count as u32, full_rebuild: true },
                        );
                        layout_engine.rebuild(&layout_input_nodes)?;
                        self.layout_full_rebuild_count += 1;
                    } else {
                        return Ok(PipelineStats { dirty_nodes: 0, layout_updates: 0, paint_misses: 0, paint_hits: 0, video_surfaces: 0 });
                    }
                }
            }
        }

        let root_id = next_ir.root.unwrap();
        // compute_layout
        let snapshot = layout_engine.compute_layout(&layout_input_nodes, root_id, viewport)?;
        // done

        let mut display_list =
            DisplayList::new(LayoutRect::new(0.0, 0.0, viewport.width, viewport.height));
        let mut paint_misses = 0;
        let mut paint_hits = 0;

        // display list generation
        self.generate_display_list_recursive(
            root_id,
            &next_ir,
            &snapshot,
            scroll_map,
            &mut display_list,
            &mut paint_misses,
            &mut paint_hits,
            video_map,
            LayoutPoint::new(0.0, 0.0),
        );
        diag::emit(
            diag::DiagCategory::Paint,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::PaintSummary {
                segments_reused: paint_hits as u32,
                segments_regenerated: paint_misses as u32,
                paint_ops_total: display_list.ops.len() as u32,
            },
        );

        renderer.render(&display_list)?;

        let video_surface_count = self.video_surfaces.len();
        self.last_snapshot = Some(snapshot);
        self.paint_cache
            .retain(|k, _| next_ir.nodes.contains_key(k));
        self.prev_ir = Some(next_ir);

        Ok(PipelineStats {
            dirty_nodes: dirty_count,
            layout_updates: dirty_count,
            paint_misses,
            paint_hits,
            video_surfaces: video_surface_count,
        })
    }

    pub fn take_video_surfaces(&mut self) -> Vec<VideoSurfaceFrame> {
        std::mem::take(&mut self.video_surfaces)
    }

    fn generate_display_list_recursive(
        &mut self,
        node_id: NodeId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        scroll_map: &ScrollStateMap,
        out_list: &mut DisplayList,
        miss_count: &mut usize,
        hit_count: &mut usize,
        video_map: &VideoStateMap,
        accumulated_offset: LayoutPoint,
    ) {
        use std::collections::HashSet;
        let mut visited = HashSet::new();
        self.generate_display_list_recursive_with_visited(
            node_id,
            ir,
            snapshot,
            scroll_map,
            out_list,
            miss_count,
            hit_count,
            video_map,
            accumulated_offset,
            &mut visited,
        );
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
        accumulated_offset: LayoutPoint,
        visited: &mut std::collections::HashSet<NodeId>,
    ) {
        if !visited.insert(node_id) {
            return;
        }
        if let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) {
            let mut hasher = DefaultHasher::new();
            node.hash.hash(&mut hasher);
            (geom.rect.origin.x.to_bits()).hash(&mut hasher);
            (geom.rect.origin.y.to_bits()).hash(&mut hasher);
            (geom.rect.size.width.to_bits()).hash(&mut hasher);
            (geom.rect.size.height.to_bits()).hash(&mut hasher);
            (geom.content_size.width.to_bits()).hash(&mut hasher);
            (geom.content_size.height.to_bits()).hash(&mut hasher);

            if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. }) = &node.op {
                let offset = scroll_map.get_offset(node_id);
                (offset.to_bits()).hash(&mut hasher);
            }

            let hash = hasher.finish();

            if let Some((cached_hash, cached_ops)) = self.paint_cache.get(&node_id) {
                if *cached_hash == hash {
                    let cached_ops = cached_ops.clone();
                    self.collect_video_surfaces(
                        node_id,
                        ir,
                        snapshot,
                        video_map,
                        scroll_map,
                        accumulated_offset,
                    );
                    *hit_count += 1;
                    for op in cached_ops {
                        out_list.push(op);
                    }
                    return;
                }
            }

            *miss_count += 1;
            let mut segment = Vec::new();

            let mut pushed_clip = false;
            let mut child_offset = accumulated_offset;

            match &node.op {
                fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) => {
                    let offset = scroll_map.get_offset(node_id);
                    segment.push(DisplayOp::Save);
                    segment.push(DisplayOp::ClipRect(geom.rect));
                    match direction {
                        fission_ir::FlexDirection::Row => {
                            segment.push(DisplayOp::Translate(LayoutPoint::new(-offset, 0.0)));
                            child_offset = LayoutPoint::new(accumulated_offset.x - offset, accumulated_offset.y);
                        }
                        fission_ir::FlexDirection::Column => {
                            segment.push(DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                            child_offset = LayoutPoint::new(accumulated_offset.x, accumulated_offset.y - offset);
                        }
                    }
                    pushed_clip = true;
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect {
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                }) => {
                    segment.push(DisplayOp::DrawRect {
                        rect: geom.rect,
                        fill: fill.map(|f| Fill {
                            color: RenderColor {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.map(|s| Stroke {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        corner_radius: *corner_radius,
                        shadow: shadow.map(|s| BoxShadow {
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
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color, underline }) => {
                    segment.push(DisplayOp::DrawText {
                        text: text.clone(),
                        position: geom.rect.origin,
                        size: *size,
                        color: fission_render::Color { r: color.r, g: color.g, b: color.b, a: color.a },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                        underline: *underline,
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage { source, fit }) => {
                    segment.push(DisplayOp::DrawImage {
                        rect: geom.rect,
                        source: source.clone(),
                        fit: match fit {
                            fission_ir::op::ImageFit::Contain => ImageFit::Contain,
                            fission_ir::op::ImageFit::Cover => ImageFit::Cover,
                            fission_ir::op::ImageFit::Fill => ImageFit::Fill,
                            fission_ir::op::ImageFit::None => ImageFit::None,
                        },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Embed {
                    kind: EmbedKind::Video,
                    widget_id,
                }) => {
                    let translated_rect = translate_rect(geom.rect, accumulated_offset);
                    self.push_video_surface(*widget_id, translated_rect, video_map);

                    segment.push(DisplayOp::DrawRect {
                        rect: translated_rect,
                        fill: Some(Fill {
                            color: RenderColor {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            },
                        }),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
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
                    child_offset,
                    visited,
                );
            }

            segment.extend(temp_dl.ops);

            if pushed_clip {
                segment.push(DisplayOp::Restore);

                if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll {
                    show_scrollbar: true,
                    ..
                }) = &node.op
                {
                    let viewport_h = geom.rect.height();
                    let content_h = geom.content_size.height;

                    if content_h > viewport_h {
                        let offset = scroll_map.get_offset(node_id);
                        let ratio = viewport_h / content_h;
                        let thumb_h = (viewport_h * ratio).max(20.0);

                        let max_scroll = content_h - viewport_h;
                        let scroll_fraction = if max_scroll > 0.0 {
                            offset / max_scroll
                        } else {
                            0.0
                        };
                        let available_track = viewport_h - thumb_h;
                        let thumb_y = available_track * scroll_fraction.clamp(0.0, 1.0);

                        let thumb_rect = LayoutRect::new(
                            geom.rect.right() - 8.0,
                            geom.rect.y() + thumb_y,
                            6.0,
                            thumb_h,
                        );

                        segment.push(DisplayOp::DrawRect {
                            rect: thumb_rect,
                            fill: Some(Fill {
                                color: RenderColor {
                                    r: 0,
                                    g: 0,
                                    b: 0,
                                    a: 100,
                                },
                            }),
                            stroke: None,
                            corner_radius: 3.0,
                            shadow: None,
                            bounds: thumb_rect,
                            node_id: None,
                        });
                    }
                }
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
            if let Some(surface_id) = state.surface_id {
                self.video_surfaces.push(VideoSurfaceFrame {
                    widget_id,
                    surface_id,
                    rect,
                });
            }
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
            SnapshotKind::Layout => {
                self.last_snapshot.as_ref().and_then(|snap| {
                    serde_json::to_string_pretty(snap)
                        .ok()
                        .map(|json| SnapshotBlob { kind, json })
                })
            }
        }
    }
}

fn detect_ir_cycle(ir: &CoreIR) -> Option<Vec<NodeId>> {
    use std::collections::{HashSet, VecDeque};
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();
    let mut path: Vec<NodeId> = Vec::new();

    fn dfs(
        ir: &CoreIR,
        node: NodeId,
        visited: &mut HashSet<NodeId>,
        stack: &mut HashSet<NodeId>,
        path: &mut Vec<NodeId>,
    ) -> Option<Vec<NodeId>> {
        if !visited.insert(node) {
            return None;
        }
        stack.insert(node);
        path.push(node);
        if let Some(n) = ir.nodes.get(&node) {
            for &child in &n.children {
                if stack.contains(&child) {
                    // Found a back edge; collect cycle path
                    let mut cycle = Vec::new();
                    // find child in path
                    if let Some(pos) = path.iter().position(|&id| id == child) {
                        cycle.extend_from_slice(&path[pos..]);
                    } else {
                        cycle.push(child);
                    }
                    return Some(cycle);
                }
                if let Some(cy) = dfs(ir, child, visited, stack, path) {
                    return Some(cy);
                }
            }
        }
        stack.remove(&node);
        path.pop();
        None
    }

    if let Some(root) = ir.root {
        if let Some(cycle) = dfs(ir, root, &mut visited, &mut stack, &mut path) {
            return Some(cycle);
        }
    }
    None
}

fn translate_rect(rect: LayoutRect, offset: LayoutPoint) -> LayoutRect {
    LayoutRect {
        origin: LayoutPoint::new(rect.origin.x + offset.x, rect.origin.y + offset.y),
        size: rect.size,
    }
}

fn detect_layout_cycle(nodes: &[fission_layout::LayoutInputNode], root: NodeId) -> Option<Vec<NodeId>> {
    use std::collections::{HashMap, HashSet};
    let map: HashMap<NodeId, &fission_layout::LayoutInputNode> = nodes.iter().map(|n| (n.id, n)).collect();
    fn dfs(
        id: NodeId,
        map: &HashMap<NodeId, &fission_layout::LayoutInputNode>,
        visited: &mut HashSet<NodeId>,
        stack: &mut HashSet<NodeId>,
        path: &mut Vec<NodeId>,
    ) -> Option<Vec<NodeId>> {
        if !visited.insert(id) { return None; }
        stack.insert(id);
        path.push(id);
        if let Some(node) = map.get(&id) {
            for child in &node.children_ids {
                if stack.contains(child) {
                    if let Some(pos) = path.iter().position(|&n| n == *child) {
                        return Some(path[pos..].to_vec());
                    } else {
                        return Some(vec![*child]);
                    }
                }
                if let Some(c) = dfs(*child, map, visited, stack, path) { return Some(c); }
            }
        }
        stack.remove(&id);
        path.pop();
        None
    }
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();
    let mut path = Vec::new();
    dfs(root, &map, &mut visited, &mut stack, &mut path)
}

fn validate_layout_invariants(nodes: &[fission_layout::LayoutInputNode], root: NodeId) -> Result<()> {
    use std::collections::{HashMap, HashSet};
    let map: HashMap<NodeId, &fission_layout::LayoutInputNode> = nodes.iter().map(|n| (n.id, n)).collect();

    // Single parent / parent-child consistency
    for n in nodes {
        for child in &n.children_ids {
            let cn = map.get(child).ok_or_else(|| anyhow::anyhow!("child {:?} missing", child))?;
            if cn.parent_id != Some(n.id) {
                return Err(anyhow::anyhow!(
                    "parent/child mismatch parent={:?} child={:?} child.parent_id={:?}\n{}",
                    n.id, child, cn.parent_id, dump_graph(nodes, 64)
                ));
            }
        }
    }

    // Cycle check
    if let Some(cycle) = detect_layout_cycle(nodes, root) {
        return Err(anyhow::anyhow!("layout cycle detected: {:?}\n{}", cycle, dump_graph(nodes, 64)));
    }

    // Reachability (warn only if orphans exist)
    let mut visited = HashSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !visited.insert(id) { continue; }
        if let Some(n) = map.get(&id) {
            for c in &n.children_ids { stack.push(*c); }
        }
    }
    // If referenced nodes exist outside visited via parent chain, that would have failed consistency above.
    Ok(())
}

fn dump_graph(nodes: &[fission_layout::LayoutInputNode], limit: usize) -> String {
    let mut lines = Vec::new();
    let mut sorted: Vec<_> = nodes.iter().collect();
    sorted.sort_by_key(|n| n.id.as_u128());
    for (i, n) in sorted.into_iter().enumerate() {
        if i >= limit { lines.push("...".into()); break; }
        let mut kids: Vec<_> = n.children_ids.iter().map(|c| format!("{:x}", c.as_u128())).collect();
        kids.sort();
        lines.push(format!("{:x} {:?} -> [{}]", n.id.as_u128(), n.op, kids.join(",")));
    }
    lines.join("\n")
}
