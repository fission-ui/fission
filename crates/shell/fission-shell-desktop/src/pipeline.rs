use fission_core::{ScrollStateMap, LayoutPoint, VideoStateMap};
use fission_ir::{CoreIR, NodeId, EmbedKind};
use fission_layout::{LayoutEngine, LayoutSize, LayoutSnapshot, LayoutRect};
use fission_render::{DisplayList, DisplayOp, Renderer, Color as RenderColor, Fill, Stroke, BoxShadow, ImageFit};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use anyhow::Result;

pub struct Pipeline {
    pub prev_ir: Option<CoreIR>,
    pub prev_snapshot: Option<LayoutSnapshot>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { prev_ir: None, prev_snapshot: None }
    }

    pub fn render(
        &mut self,
        next_ir: CoreIR,
        viewport: LayoutSize,
        layout_engine: &LayoutEngine,
        scroll_map: &ScrollStateMap,
        video_map: &VideoStateMap,
        renderer: &mut impl Renderer
    ) -> Result<()> {
        let can_reuse_layout = if let (Some(prev_ir), Some(prev_layout)) = (&self.prev_ir, &self.prev_snapshot) {
            if let (Some(root), Some(prev_root)) = (next_ir.root, prev_ir.root) {
                if let (Some(next_node), Some(prev_node)) = (next_ir.nodes.get(&root), prev_ir.nodes.get(&prev_root)) {
                     next_node.hash == prev_node.hash && prev_layout.viewport_size == viewport
                } else { false }
            } else { false }
        } else { false };

        let snapshot = if can_reuse_layout {
            self.prev_snapshot.clone().unwrap()
        } else {
            let layout_input_nodes = build_layout_tree(&next_ir);
            if let Some(root_id) = next_ir.root {
                layout_engine.compute_layout(&layout_input_nodes, root_id, viewport)?
            } else {
                return Ok(());
            }
        };

        let mut display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, viewport.width, viewport.height));
        if let Some(root_id) = next_ir.root {
             generate_display_list(root_id, &next_ir, &snapshot, scroll_map, video_map, &mut display_list);
        }
        
        renderer.render(&display_list)?;
        
        self.prev_ir = Some(next_ir);
        self.prev_snapshot = Some(snapshot);
        
        Ok(())
    }
}

fn generate_display_list(
    node_id: NodeId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    video_map: &VideoStateMap,
    list: &mut DisplayList
) {
    if let Some(geom) = snapshot.nodes.get(&node_id) {
        if let Some(node) = ir.nodes.get(&node_id) {
            let mut pushed_clip = false;

            match &node.op {
                fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { show_scrollbar, .. }) => {
                    let offset = scroll_map.get_offset(node_id);
                    list.push(DisplayOp::Save);
                    list.push(DisplayOp::ClipRect(geom.rect));
                    list.push(DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                    pushed_clip = true;
                },
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect { fill, stroke, corner_radius, shadow }) => {
                    list.push(DisplayOp::DrawRect { 
                        rect: geom.rect,
                        fill: fill.map(|f| Fill { color: RenderColor { r: f.color.r, g: f.color.g, b: f.color.b, a: f.color.a } }),
                        stroke: stroke.map(|s| Stroke { color: RenderColor { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, width: s.width }),
                        corner_radius: *corner_radius,
                        shadow: shadow.map(|s| BoxShadow { 
                            color: RenderColor { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, 
                            blur_radius: s.blur_radius, 
                            offset: s.offset 
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                },
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color }) => {
                    list.push(DisplayOp::DrawText { 
                        text: text.clone(),
                        position: geom.rect.origin, 
                        size: *size,
                        color: RenderColor { r: color.r, g: color.g, b: color.b, a: color.a },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                },
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage { source, fit }) => {
                    list.push(DisplayOp::DrawImage { 
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
                },
                fission_ir::Op::Layout(fission_ir::LayoutOp::Embed { kind: EmbedKind::Video }) => {
                     let surface_id = video_map.states.get(&node_id).and_then(|s| s.surface_id).unwrap_or(0);
                     list.push(DisplayOp::DrawSurface {
                         rect: geom.rect,
                         surface_id,
                         bounds: geom.rect,
                         node_id: Some(node_id),
                     });
                },
                _ => {}
            }

            for child in &node.children {
                generate_display_list(*child, ir, snapshot, scroll_map, video_map, list);
            }

            if pushed_clip {
                list.push(DisplayOp::Restore);
                
                if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { show_scrollbar: true, .. }) = &node.op {
                    let viewport_h = geom.rect.height();
                    let content_h = geom.content_size.height;
                    
                    if content_h > viewport_h {
                        let offset = scroll_map.get_offset(node_id);
                        let ratio = viewport_h / content_h;
                        let thumb_h = (viewport_h * ratio).max(20.0);
                        
                        let max_scroll = content_h - viewport_h;
                        let scroll_fraction = if max_scroll > 0.0 { offset / max_scroll } else { 0.0 };
                        let available_track = viewport_h - thumb_h;
                        let thumb_y = available_track * scroll_fraction.clamp(0.0, 1.0);
                        
                        let thumb_rect = LayoutRect::new(
                            geom.rect.right() - 8.0, 
                            geom.rect.y() + thumb_y,
                            6.0,
                            thumb_h
                        );
                        
                        list.push(DisplayOp::DrawRect {
                            rect: thumb_rect,
                            fill: Some(Fill { color: RenderColor { r: 0, g: 0, b: 0, a: 100 } }),
                            stroke: None,
                            corner_radius: 3.0,
                            shadow: None,
                            bounds: thumb_rect,
                            node_id: None,
                        });
                    }
                }
            }
        }
    }
}