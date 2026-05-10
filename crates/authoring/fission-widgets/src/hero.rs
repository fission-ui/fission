use fission_core::{BuildCtx, LowerDyn, LoweringContext, Node, NodeBuilder, NodeId, View, Widget};
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
            identifier: None,
            value: None,
            actions: Default::default(),
            focusable: false,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
            read_only: false,
            autofocus: false,
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
            focus_index: None,
            text_input_type: fission_ir::semantics::TextInputType::Text,
            text_input_action: fission_ir::semantics::TextInputAction::Done,
            text_capitalization: fission_ir::semantics::TextCapitalization::None,
            max_length: None,
            max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
            input_formatters: Vec::new(),
            autocorrect: true,
            enable_suggestions: true,
            spell_check: true,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
            capture_tab: false,
            auto_indent: false,
            scroll_padding: None,
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
