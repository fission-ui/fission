use fission_core::internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx};
use fission_core::Widget;
use fission_ir::WidgetId;
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
    pub child: Widget,
}

impl From<Hero> for Widget {
    fn from(component: Hero) -> Self {
        let this = &component;

        fission_core::internal::custom_render_widget(fission_core::internal::InternalRenderNode {
            debug_tag: format!("Hero({})", this.tag),
            lowerer: Some(std::sync::Arc::new(HeroLowerer {
                tag: this.tag.clone(),
                child: this.child.clone(),
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct HeroLowerer {
    tag: String,
    child: Widget,
}

impl InternalLowerer for HeroLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let child_id = fission_core::internal::lower_widget(&self.child, cx);
        let id = cx.next_node_id();

        let semantics = Semantics {
            role: Role::Generic,
            hero_tag: Some(self.tag.clone()),
            ..Semantics::default()
        };

        let mut builder = InternalIrBuilder::new(id, Op::Semantics(semantics));
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
