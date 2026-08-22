//! Retained, zoomable two-dimensional node canvas.
//!
//! `InfiniteCanvas` composes ordinary Fission widgets on top of
//! [`InteractiveViewer`]. Node and edge data remain declarative application
//! state; the viewer owns only transient camera interaction.

mod edges;
mod geometry;
mod grid;
mod interaction_region;
mod model;
mod nodes;
mod overlay;
mod vector_layer;

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use fission_core::internal::{InternalLowerer, InternalLoweringCx, InternalRenderNode};
use fission_core::ui::{
    InteractiveViewer, ViewportBoundary, ViewportClip, ViewportPanAxis, ViewportTransform,
    ViewportZoomPolicy,
};
use fission_core::ui::{Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{CanvasTarget, CanvasTargetKind};
use fission_layout::LayoutRect;
use serde::{Deserialize, Serialize};

use edges::InfiniteCanvasEdgeLayer;
use geometry::visible_world_rect;
use grid::InfiniteCanvasGridLayer;
use interaction_region::CanvasInteractionRegion;
use nodes::InfiniteCanvasNodeLayer;
use overlay::InfiniteCanvasOverlay;
use vector_layer::CanvasVectorLayer;

pub use model::{
    CanvasEdgeEndpoint, CanvasEdgeId, CanvasEdgeRoute, CanvasGrid, CanvasNodeAnchor, CanvasNodeId,
    CanvasSelectionPolicy, CanvasSnap, InfiniteCanvasActions, InfiniteCanvasEdge,
    InfiniteCanvasNode,
};

/// A declarative infinite node canvas built on [`InteractiveViewer`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfiniteCanvas {
    pub id: Option<WidgetId>,
    pub nodes: Vec<InfiniteCanvasNode>,
    pub edges: Vec<InfiniteCanvasEdge>,
    pub initial_transform: ViewportTransform,
    pub transform: Option<ViewportTransform>,
    pub selected_nodes: BTreeSet<CanvasNodeId>,
    pub selected_edges: BTreeSet<CanvasEdgeId>,
    pub selection_policy: CanvasSelectionPolicy,
    pub snap: CanvasSnap,
    pub grid: Option<CanvasGrid>,
    /// Active marquee in screen coordinates, when controlled by the app.
    pub marquee_screen_rect: Option<LayoutRect>,
    pub pan_axis: ViewportPanAxis,
    pub boundary: ViewportBoundary,
    pub clip: ViewportClip,
    pub zoom_policy: ViewportZoomPolicy,
    pub min_scale: f32,
    pub max_scale: f32,
    pub friction: f32,
    pub render_overscan: f32,
    pub actions: InfiniteCanvasActions,
}

impl InfiniteCanvas {
    pub fn new(nodes: Vec<InfiniteCanvasNode>) -> Self {
        Self {
            nodes,
            ..Self::default()
        }
    }
}

impl Default for InfiniteCanvas {
    fn default() -> Self {
        Self {
            id: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            initial_transform: ViewportTransform::IDENTITY,
            transform: None,
            selected_nodes: BTreeSet::new(),
            selected_edges: BTreeSet::new(),
            selection_policy: CanvasSelectionPolicy::Single,
            snap: CanvasSnap::default(),
            grid: None,
            marquee_screen_rect: None,
            pan_axis: ViewportPanAxis::Both,
            boundary: ViewportBoundary::Unbounded,
            clip: ViewportClip::HardEdge,
            zoom_policy: ViewportZoomPolicy::WheelWithModifier,
            min_scale: fission_core::ui::DEFAULT_MIN_VIEWPORT_SCALE,
            max_scale: fission_core::ui::DEFAULT_MAX_VIEWPORT_SCALE,
            friction: fission_core::ui::DEFAULT_VIEWPORT_FRICTION,
            render_overscan: 128.0,
            actions: InfiniteCanvasActions::default(),
        }
    }
}

impl From<InfiniteCanvas> for Widget {
    fn from(canvas: InfiniteCanvas) -> Self {
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "InfiniteCanvas".into(),
            lowerer: Some(Arc::new(canvas)),
            render_object: None,
        })
    }
}

impl InternalLowerer for InfiniteCanvas {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let resolved = self.resolved_widget(cx);
        fission_core::internal::lower_widget(&resolved, cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        let canvas_id = self
            .id
            .unwrap_or_else(|| WidgetId::explicit("fission.infinite-canvas"));
        Some(WidgetId::derived(canvas_id.as_u128(), &[0xCA4A5]))
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{self:?}").hash(&mut hasher);
        hasher.finish()
    }
}

impl InfiniteCanvas {
    fn resolved_widget(&self, cx: &InternalLoweringCx<'_>) -> Widget {
        let canvas_id = self
            .id
            .unwrap_or_else(|| WidgetId::explicit("fission.infinite-canvas"));
        let transform = self
            .transform
            .or_else(|| cx.runtime_state.viewport.transform(canvas_id))
            .unwrap_or(self.initial_transform)
            .normalized();
        let viewport = cx
            .layout
            .and_then(|layout| layout.get_node_rect(canvas_id))
            .map(|rect| rect.size)
            .unwrap_or(cx.env.viewport_size);
        let visible_world = visible_world_rect(
            viewport.width,
            viewport.height,
            transform.translation[0],
            transform.translation[1],
            transform.scale,
            self.render_overscan.max(0.0),
        );
        let selection_color = cx.env.theme.tokens.colors.primary;

        let mut world_children = Vec::new();
        if let Some(grid) = self.grid {
            world_children.push(
                InfiniteCanvasGridLayer {
                    canvas_id,
                    grid,
                    visible_world,
                }
                .into(),
            );
        }
        world_children.push(
            InfiniteCanvasEdgeLayer {
                canvas_id,
                edges: self.edges.clone(),
                nodes: self.nodes.clone(),
                selected_edges: self.selected_edges.clone(),
                visible_world,
                selection_policy: self.selection_policy,
                snap: self.snap,
                on_edge_selection: self.actions.on_edge_selection.clone(),
            }
            .into(),
        );
        world_children.push(
            InfiniteCanvasNodeLayer {
                canvas_id,
                nodes: self.nodes.clone(),
                selection: self.selected_nodes.clone(),
                selection_color,
                selection_policy: self.selection_policy,
                snap: self.snap,
                on_selection_change: self.actions.on_selection_change.clone(),
                on_node_move: self.actions.on_node_move.clone(),
            }
            .into(),
        );

        let world: Widget = ZStack {
            id: Some(WidgetId::derived(canvas_id.as_u128(), &[0xC001D])),
            children: world_children,
        }
        .into();
        let world = self.selection_region(canvas_id, world);
        let viewer: Widget = InteractiveViewer {
            id: Some(canvas_id),
            child: world,
            initial_transform: self.initial_transform,
            transform: self.transform,
            pan_axis: self.pan_axis,
            boundary: self.boundary,
            clip: self.clip,
            zoom_policy: self.zoom_policy,
            min_scale: self.min_scale,
            max_scale: self.max_scale,
            friction: self.friction,
            on_interaction_start: self.actions.on_interaction_start.clone(),
            on_interaction_update: self.actions.on_interaction_update.clone(),
            on_interaction_end: self.actions.on_interaction_end.clone(),
        }
        .into();

        let overlay: Widget = InfiniteCanvasOverlay {
            canvas_id,
            nodes: self.nodes.clone(),
            selection: self.selected_nodes.clone(),
            marquee: self.marquee_screen_rect,
            translation_x: transform.translation[0],
            translation_y: transform.translation[1],
            scale: transform.scale,
            color: selection_color,
            selection_policy: self.selection_policy,
            snap: self.snap,
            on_node_resize: self.actions.on_node_resize.clone(),
        }
        .into();

        ZStack {
            id: Some(WidgetId::derived(canvas_id.as_u128(), &[0xCA4A5, 0])),
            children: vec![viewer, overlay],
        }
        .into()
    }

    fn selection_region(&self, canvas_id: WidgetId, child: Widget) -> Widget {
        let (on_activate, on_drag): (Option<ActionEnvelope>, Option<ActionEnvelope>) =
            match self.selection_policy {
                CanvasSelectionPolicy::None => (None, None),
                CanvasSelectionPolicy::Single | CanvasSelectionPolicy::Toggle => {
                    (self.actions.on_selection_change.clone(), None)
                }
                CanvasSelectionPolicy::Marquee => (
                    self.actions.on_selection_change.clone(),
                    self.actions.on_selection_change.clone(),
                ),
            };
        CanvasInteractionRegion {
            id: WidgetId::derived(canvas_id.as_u128(), &[0x5E1EC7]),
            child,
            identifier: format!("infinite-canvas:{:032x}:selection", canvas_id.as_u128()),
            target: CanvasTarget {
                canvas_id: canvas_id.as_u128(),
                kind: CanvasTargetKind::Marquee,
                selection_policy: self.selection_policy,
                snap_spacing: self.snap.enabled.then_some(self.snap.spacing),
                snap_threshold: self.snap.threshold,
            },
            on_activate,
            on_drag,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::env::{Env, RuntimeState};
    use fission_core::ui::Spacer;

    #[test]
    fn snapping_is_symmetric_for_negative_world_coordinates() {
        let snap = CanvasSnap {
            enabled: true,
            spacing: 10.0,
            threshold: 3.0,
        };
        assert_eq!(snap.snap(-18.0), -20.0);
        assert_eq!(snap.snap(-14.0), -14.0);
        assert_eq!(snap.snap(22.0), 20.0);
    }

    #[test]
    fn graphical_defaults_are_unbounded_and_declarative() {
        let canvas = InfiniteCanvas::default();
        assert_eq!(canvas.initial_transform, ViewportTransform::IDENTITY);
        assert_eq!(canvas.transform, None);
        assert_eq!(canvas.boundary, ViewportBoundary::Unbounded);
        assert_eq!(canvas.selection_policy, CanvasSelectionPolicy::Single);
    }

    #[test]
    fn complete_canvas_lowers_with_distinct_stable_wrapper_ids() {
        let canvas_id = WidgetId::explicit("lowered-canvas");
        let widget: Widget = InfiniteCanvas {
            id: Some(canvas_id),
            nodes: vec![InfiniteCanvasNode::new(
                CanvasNodeId::from_u128(7),
                LayoutRect::new(-20.0, 10.0, 80.0, 40.0),
                Spacer::default(),
            )],
            ..Default::default()
        }
        .into();
        let env = Env::default();
        let runtime = RuntimeState::default();
        let mut cx = fission_core::internal::InternalLoweringCx::new(&env, &runtime, None, None);

        let root = fission_core::internal::lower_widget(&widget, &mut cx);

        assert_eq!(root, WidgetId::derived(canvas_id.as_u128(), &[0xCA4A5]));
        let wrapper = cx.ir.nodes.get(&root).expect("canvas wrapper");
        assert_eq!(wrapper.children.len(), 1);
        assert_ne!(wrapper.children[0], root);
        assert!(cx.ir.nodes.contains_key(&canvas_id));
    }
}
