use fission_core::ui::{Container, Node};
use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, ActionEnvelope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Popover {
    pub id: WidgetNodeId,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
    pub on_close: Option<ActionEnvelope>,
    
    pub trigger: Box<Node>,
    pub content: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for Popover {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        // Derive stable anchor ID
        let anchor_id = NodeId::derived(self.id.as_u128(), &[0]);
        
        let trigger_wrapper = Container::new(*self.trigger.clone())
            .id(anchor_id)
            .into_node();
            
        // Wrap trigger in a clickable area if on_toggle provided?
        // Or assume trigger handles clicks.
        // Usually trigger handles clicks.
        
        if self.is_open {
            let content_node = *self.content.clone();
            
            // Backdrop to handle click-outside (on_close)
            // But Flyout positioning is special.
            // If we use Backdrop (AbsoluteFill), it covers screen.
            // Then Flyout (Absolute) sits on top?
            // Yes.
            
            use fission_core::ui::{Button, ButtonVariant};
            use fission_core::op::Color;
            
            let backdrop = Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                        .bg(Color { r: 0, g: 0, b: 0, a: 0 }) // Transparent
                        .into_node()
                )),
                on_press: self.on_close.clone(),
                ..Default::default()
            }.into_node();
            
            let flyout_node = crate::flyout(anchor_id, content_node);
            
            // We need to render [Backdrop, Flyout].
            // Backdrop is ZStack layer 0. Flyout layer 1.
            use fission_core::ui::ZStack;
            
            let overlay = ZStack {
                children: vec![
                    // Backdrop must fill screen. 
                    // Lowering of Button -> Box -> Flex.
                    // We need it to be AbsoluteFill.
                    // Wrap button in AbsoluteFill container?
                    // Or Container with AbsoluteFill op?
                    // fission-widgets::Portal wraps child.
                    // If child is ZStack, it's fine.
                    // How to make Backdrop fill screen?
                    // Use Positioned { left:0, top:0, bottom:0, right:0 }.
                    fission_core::ui::Positioned {
                        left: Some(0.0), top: Some(0.0), right: Some(0.0), bottom: Some(0.0),
                        child: Some(Box::new(backdrop)),
                        ..Default::default()
                    }.into_node(),
                    
                    flyout_node
                ],
                ..Default::default()
            }.into_node();
            
            ctx.register_portal(overlay);
        }
        
        trigger_wrapper
    }
}