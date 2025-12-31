use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler};
use fission_core::ui::Node;
use fission_widgets::{Modal, ModalAction, VStack, TextInput, FormControl};
use crate::model::{InboxState, ToggleCompose, ToggleToast};

pub struct ComposeModal;

impl Widget<InboxState> for ComposeModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Modal {
            id: WidgetNodeId::explicit("compose_modal"),
            title: "New Message".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleCompose, (|s: &mut InboxState, _: ToggleCompose, _| s.show_compose = false) as Handler<InboxState, ToggleCompose>)),
            width: Some(600.0),
            content: Box::new(
                VStack {
                    spacing: Some(12.0),
                    children: vec![
                        FormControl {
                            id: None,
                            label: Some("To".into()),
                            required: true,
                            error: None,
                            helper: None,
                            child: Box::new(TextInput { placeholder: Some("recipient@example.com".into()), width: Some(550.0), ..Default::default() }.into_node()),
                        }.build(ctx, view),
                        
                        FormControl {
                            id: None,
                            label: Some("Subject".into()),
                            required: false,
                            error: None,
                            helper: None,
                            child: Box::new(TextInput { placeholder: Some("Subject".into()), width: Some(550.0), ..Default::default() }.into_node()),
                        }.build(ctx, view),
                        
                        FormControl {
                            id: None,
                            label: Some("Message".into()),
                            required: true,
                            error: None,
                            helper: Some("Markdown supported".into()),
                            child: Box::new(TextInput { 
                                placeholder: Some("Type your message...".into()), 
                                multiline: true,
                                width: Some(550.0),
                                height: Some(200.0),
                                ..Default::default() 
                            }.into_node()),
                        }.build(ctx, view),
                    ]
                }.into_node()
            ),
            actions: vec![
                ModalAction { label: "Cancel".into(), is_primary: false, on_press: Some(ctx.bind(ToggleCompose, (|s: &mut InboxState, _: ToggleCompose, _| s.show_compose = false) as Handler<InboxState, ToggleCompose>)) },
                ModalAction { 
                    label: "Send".into(), 
                    is_primary: true, 
                    on_press: Some(ctx.bind(ToggleToast(true), (|s: &mut InboxState, _: ToggleToast, _| { s.show_compose = false; s.show_toast = true; }) as Handler<InboxState, ToggleToast>)) 
                },
            ]
        }.build(ctx, view)
    }
}