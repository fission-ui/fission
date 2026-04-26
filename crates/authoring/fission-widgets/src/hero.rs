use fission_core::{
    BuildCtx, Lower, LowerDyn, LoweringContext, Node, NodeBuilder, NodeId, View, Widget,
};
use fission_ir::{semantics::Role, Op, Semantics};
use serde::{Deserialize, Serialize};

/// A shared-element transition tag for cross-navigation animations.
///
/// Wraps a child widget with a `hero_tag` semantic annotation. When two `Hero`
/// widgets with the same `tag` appear in consecutive navigation frames, the
/// framework can animate the element's position and size between the two locations.
///
/// # Fields
///
/// * `tag` - A unique string identifying this hero element across routes.
/// * `child` - The widget to animate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hero {
    pub tag: String,
    pub child: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for Hero {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Custom(fission_core::ui::CustomNode {
            debug_tag: format!("Hero({})", self.tag),
            lowerer: Some(std::sync::Arc::new(HeroLowerer {
                tag: self.tag.clone(),
                child: *self.child.clone(),
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct HeroLowerer {
    tag: String,
    child: Node,
}

impl LowerDyn for HeroLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let child_id = self.child.lower(cx);
        let id = cx.next_node_id();

        let semantics = Semantics {
            role: Role::Generic,
            label: None,
            value: None,
            actions: Default::default(),
            focusable: false,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
            draggable: false,
            scrollable_x: false,
            scrollable_y: false,
            min_value: None,
            max_value: None,
            current_value: None,
            is_focus_scope: false,
            is_focus_barrier: false,
            drag_payload: None,
            hero_tag: Some(self.tag.clone()),
            focus_index: None, capture_tab: false, auto_indent: false,
        };

        let mut builder = NodeBuilder::new(id, Op::Semantics(semantics));
        builder.add_child(child_id);
        builder.build(cx)
    }

    fn stable_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.tag.hash(&mut h);
        h.finish()
    }
}
