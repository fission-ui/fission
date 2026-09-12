mod layout_policy;
pub mod op;
pub mod semantics;
mod text_style;
pub mod viewport;
pub mod widget_id;

#[cfg(test)]
mod op_tests;

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

pub use op::{
    AlignItems, BackdropFilter, BlendMode, CompositeScalar, CompositeStyle, EmbedKind,
    FlexDirection, FlexWrap, FlyoutAlignment, FlyoutOptions, FlyoutPlacement, FlyoutWidth,
    GridPlacement, GridTrack, JustifyContent, LayoutDirection, LayoutOp, Op, PaintOp, StructuralOp,
    ViewportBoundary, ViewportClip, ViewportMargin, ViewportPanAxis, ViewportTransform,
    ViewportZoomPolicy,
};
pub use semantics::{
    ActionEntry, ActionSet, ActionTrigger, CanvasNodeMoveTarget, CanvasSelectionPolicy,
    CanvasTarget, CanvasTargetKind, FocusPolicy, Hyperlink, InputFormatter, LinkTarget,
    MaxLengthEnforcement, PopoverAction, PopoverTarget, PopupKind, Role, SelectionRegionSemantics,
    SemanticOrientation, Semantics, TextCapitalization, TextFieldValidationState, TextInputAction,
    TextInputType, TextWrapMode,
};
pub use widget_id::WidgetId;

/// Version of the serialized core IR schema.
///
/// Version 2 adds composite-control semantics, logical layout direction,
/// interaction-inert structure, and configurable flyout policy.
pub const IR_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreNode {
    pub id: WidgetId,
    pub op: Op,
    pub composite: CompositeStyle,
    pub children: Vec<WidgetId>,
    pub parent: Option<WidgetId>,
    pub hash: u64,
}

/// A type-erased render object stored alongside IR nodes.
///
/// Downstream crates (e.g. `fission-core`) store concrete trait objects here
/// (typically `Arc<dyn CustomRenderObject>`).  `fission-ir` itself never
/// inspects these values -- it only provides the storage.
pub type AnyRenderObject = Arc<dyn Any + Send + Sync>;

#[derive(Clone, Serialize, Deserialize)]
pub struct CoreIR {
    pub nodes: HashMap<WidgetId, CoreNode>,
    pub root: Option<WidgetId>,
    /// Direction snapshot derived from the application environment.
    ///
    /// Logical widget order remains unchanged; layout and interaction use this
    /// value to resolve physical left/right behavior. The default is omitted so
    /// existing left-to-right serialized IR remains byte-for-byte compatible.
    #[serde(default, skip_serializing_if = "LayoutDirection::is_left_to_right")]
    pub layout_direction: LayoutDirection,
    /// Per-widget custom render objects. Keyed by the wrapper `WidgetId` created
    /// during lowering of an `InternalRenderNode`. Skipped by serde because the
    /// concrete trait objects are not serialisable.
    #[serde(skip)]
    pub custom_render_objects: HashMap<WidgetId, AnyRenderObject>,
}

impl std::fmt::Debug for CoreIR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreIR")
            .field("nodes", &self.nodes)
            .field("root", &self.root)
            .field("layout_direction", &self.layout_direction)
            .field(
                "custom_render_objects",
                &format!("({} entries)", self.custom_render_objects.len()),
            )
            .finish()
    }
}

impl PartialEq for CoreIR {
    fn eq(&self, other: &Self) -> bool {
        // custom_render_objects are intentionally excluded from equality --
        // they are ephemeral, non-serialisable extensions.
        self.nodes == other.nodes
            && self.root == other.root
            && self.layout_direction == other.layout_direction
    }
}

impl Default for CoreIR {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            layout_direction: LayoutDirection::default(),
            custom_render_objects: HashMap::new(),
        }
    }
}

impl CoreIR {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, id: WidgetId, op: Op, children: Vec<WidgetId>) {
        self.add_node_with_composite(id, op, CompositeStyle::default(), children);
    }

    pub fn add_node_with_composite(
        &mut self,
        id: WidgetId,
        op: Op,
        composite: CompositeStyle,
        children: Vec<WidgetId>,
    ) {
        let core_node = CoreNode {
            id,
            op,
            composite,
            children: children.clone(),
            parent: None,
            hash: 0,
        };
        self.nodes.insert(id, core_node);

        for child_id in children {
            if let Some(child_node) = self.nodes.get_mut(&child_id) {
                child_node.parent = Some(id);
            }
        }
    }

    pub fn set_root(&mut self, id: WidgetId) {
        self.root = Some(id);
    }
}
