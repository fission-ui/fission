use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use fission_core::ui::Node;
use fission_widgets::{Modal, ModalAction, VStack, TextInput};
use crate::model::{InboxState, ToggleCompose, ToggleToast};

pub struct ComposeModal;

impl Widget<InboxState> for ComposeModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Modal {
            id: WidgetNodeId::explicit("compose_modal"),
            title: "New Message".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleCompose, |s, _| s.show_compose = false)),
            width: Some(600.0),
            content: Box::new(
                VStack {
                    spacing: Some(12.0),
                    children: vec![
                        TextInput { placeholder: Some("To".into()), width: Some(550.0), ..Default::default() }.into_node(),
                        TextInput { placeholder: Some("Subject".into()), width: Some(550.0), ..Default::default() }.into_node(),
                        TextInput { 
                            placeholder: Some("Message...".into()), 
                            multiline: true,
                            width: Some(550.0),
                            height: Some(200.0),
                            ..Default::default() 
                        }.into_node(),
                    ]
                }.into_node()
            ),
            actions: vec![
                ModalAction { label: "Cancel".into(), is_primary: false, on_press: Some(ctx.bind(ToggleCompose, |s, _| s.show_compose = false)) },
                ModalAction { 
                    label: "Send".into(), 
                    is_primary: true, 
                    on_press: Some(ctx.bind(ToggleToast(true), |s, _| { s.show_compose = false; s.show_toast = true; })) 
                },
            ]
        }.build(ctx, view)
    }
}