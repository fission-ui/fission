use fission_core::authoring::{IrBuilder, LowerWidget, LoweringContext};
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
    /// Stable shared-element name that identifies the same visual across routes.
    pub tag: String,
    /// Element whose bounds and appearance participate in the transition.
    pub child: Widget,
}

impl From<Hero> for Widget {
    fn from(component: Hero) -> Self {
        let this = &component;

        fission_core::authoring::custom_widget(
            format!("Hero({})", this.tag),
            HeroLowerer {
                tag: this.tag.clone(),
                child: this.child.clone(),
            },
        )
    }
}

#[derive(Debug)]
struct HeroLowerer {
    tag: String,
    child: Widget,
}

impl LowerWidget for HeroLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let child_id = fission_core::internal::lower_widget(&self.child, cx);
        let id = cx.next_node_id();

        let semantics = Semantics {
            role: Role::Generic,
            hero_tag: Some(self.tag.clone()),
            ..Semantics::default()
        };

        let mut builder = IrBuilder::new(id, Op::Semantics(semantics));
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
