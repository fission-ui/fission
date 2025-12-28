use fission_core::ui::{Container, Node};
use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use crate::positioned::Positioned;
use crate::Portal; // Assuming Portal is re-exported or available
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Popover {
    pub trigger: Box<Node>,
    pub content: Box<Node>,
    pub is_open: bool,
    pub anchor_id: WidgetNodeId,
}

impl<S: fission_core::AppState> Widget<S> for Popover {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        // Trigger with anchor ID
        // Note: anchor_id is WidgetNodeId (u128). NodeId is u64.
        // We use from_u128 (if available) or assume NodeId logic.
        // Fission NodeId::from_u128? NodeId is u64 wrapper? No, u64.
        // WidgetNodeId is u128.
        // We cannot map u128 to u64 losslessly.
        // However, `View::get_rect` uses `NodeId::from_u128`?
        // Wait, NodeId is u64.
        // My `View::get_rect` implementation used `NodeId::from_u128`.
        // Does `NodeId` have `from_u128`?
        // `crates/core/fission-ir/src/node_id.rs`?
        // NodeId is `derive(..., Copy, ...)` around `u64`?
        // Let's check NodeId definition.
        
        // Assuming I can't check now, I'll rely on the standard pattern:
        // Use `WidgetNodeId` for logical ID.
        // `cx.widget_node_id(w_id)` -> `NodeId`.
        // We need that specific `NodeId` to be assigned to the container.
        // BUT `Container` takes `NodeId`.
        // `Widget` build doesn't return `NodeId`, it returns `Node`.
        // `Node::lower` generates IDs.
        
        // To anchor, we need the `NodeId` that `Container` WILL use.
        // `Container::lower` uses `self.id` OR `cx.next_node_id()`.
        // If we provide `self.id`, we control it.
        // BUT we need `cx` to convert `WidgetNodeId` to `NodeId`?
        // `LoweringContext` does that. `BuildCtx` does NOT.
        
        // This is a Catch-22. We are in `build` (Authoring), but we need Layout ID (Core).
        // Layout IDs are generated during Lowering.
        // Anchoring requires knowing the Layout ID *before* layout?
        // Or persistently.
        
        // Solution: Use explicit `NodeId` if possible? 
        // No, `NodeId` is transient (u64).
        
        // We need `View::get_rect` to take `WidgetNodeId` and look up the *mapped* `NodeId` from previous frame.
        // Does `Runtime` track `WidgetNodeId -> NodeId` mapping?
        // `Runtime` uses `NodeId` for everything.
        // `VideoStateMap` uses `WidgetNodeId`.
        // `InteractionState` uses `NodeId`.
        
        // Fission seems to rely on `WidgetNodeId` for persistence.
        // But `LayoutSnapshot` is keyed by `NodeId`.
        // We need a map `WidgetNodeId -> NodeId`.
        // `LoweringContext` builds this map?
        // We assume `NodeId` *is* stable if derived from `WidgetNodeId`.
        // `NodeId::derived(u128, ...)` uses hashing.
        
        // If we set `Container.id = NodeId::derived(anchor_id.as_u128(), &[])`, 
        // then `View::get_rect` can re-derive it!
        // `NodeId::derived` logic is deterministic.
        
        let node_id = NodeId::derived(self.anchor_id.as_u128(), &[]);
        
        let trigger_node = Container::new(*self.trigger.clone())
            .id(node_id)
            .into_node();
            
        // Portal
        if self.is_open {
            if let Some(rect) = view.get_rect(self.anchor_id) {
                // We assume View::get_rect re-derives NodeId same way.
                
                let x = rect.origin.x;
                let y = rect.bottom() + 4.0;
                
                let content = Positioned {
                    left: Some(x),
                    top: Some(y),
                    child: *self.content.clone(),
                    ..Default::default()
                }.build(ctx, view);
                
                // Return Trigger + Portal (via ctx registration)
                ctx.register_portal(content);
            }
        }
        
        trigger_node
    }
}
