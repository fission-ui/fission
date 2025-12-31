use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler, ActionEnvelope, ActionId};
use fission_core::ui::Node;
use fission_widgets::{Modal, ModalAction, VStack, HStack, TextInput, FormControl, Combobox, DatePicker, TimePicker, FileUpload, Dropzone};
use crate::model::{InboxState, ToggleCompose, ToggleToast, SetComposeTo, SetScheduleDate, SetScheduleTime, ToggleDatePicker, FileSelected};
use std::sync::Arc;
use serde_json;

pub struct ComposeModal;

impl Widget<InboxState> for ComposeModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        // Register Handlers
        let to_id = ctx.bind(SetComposeTo("".into()), (|s: &mut InboxState, a: SetComposeTo, _| s.compose_to = a.0) as Handler<InboxState, SetComposeTo>).id;
        let date_id = ctx.bind(SetScheduleDate(chrono::Local::now().date_naive()), (|s: &mut InboxState, a: SetScheduleDate, _| s.schedule_date = Some(a.0)) as Handler<InboxState, SetScheduleDate>).id;
        let time_id = ctx.bind(SetScheduleTime(0, 0), (|s: &mut InboxState, a: SetScheduleTime, _| s.schedule_time = Some((a.0, a.1))) as Handler<InboxState, SetScheduleTime>).id;

        let content = VStack {
            spacing: Some(12.0),
            children: vec![
                // To (Combobox)
                FormControl {
                    id: None,
                    label: Some("To".into()),
                    required: true,
                    error: None,
                    helper: None,
                    child: Box::new(Combobox {
                        id: WidgetNodeId::explicit("compose_to"),
                        value: view.state.compose_to.clone(),
                        items: vec!["alice@example.com".into(), "bob@example.com".into(), "team@fission.rs".into()], 
                        is_open: !view.state.compose_to.is_empty(), 
                        on_change: Some(ActionEnvelope {
                            id: to_id,
                            payload: Vec::new(),
                        }),
                        on_select: Some(Arc::new(move |val| {
                            ActionEnvelope {
                                id: to_id,
                                payload: serde_json::to_vec(&SetComposeTo(val)).unwrap(),
                            }
                        })),
                        on_toggle: None,
                    }.build(ctx, view)),
                }.build(ctx, view),
                
                // Subject
                FormControl {
                    id: None,
                    label: Some("Subject".into()),
                    required: false,
                    error: None,
                    helper: None,
                    child: Box::new(TextInput { 
                        value: view.state.compose_subject.clone(),
                        placeholder: Some("Subject".into()), 
                        width: Some(550.0), 
                        ..Default::default() 
                    }.into_node()),
                }.build(ctx, view),
                
                // Schedule
                HStack {
                    spacing: Some(12.0),
                    children: vec![
                        DatePicker {
                            id: WidgetNodeId::explicit("schedule_date"),
                            value: view.state.schedule_date,
                            is_open: view.state.is_date_picker_open,
                            on_change: Some(Arc::new(move |d| {
                                ActionEnvelope {
                                    id: date_id,
                                    payload: serde_json::to_vec(&SetScheduleDate(d)).unwrap(),
                                }
                            })),
                            on_toggle: Some(ctx.bind(ToggleDatePicker, (|s: &mut InboxState, _: ToggleDatePicker, _| s.is_date_picker_open = !s.is_date_picker_open) as Handler<InboxState, ToggleDatePicker>)),
                            on_close: Some(ctx.bind(ToggleDatePicker, (|s: &mut InboxState, _: ToggleDatePicker, _| s.is_date_picker_open = false) as Handler<InboxState, ToggleDatePicker>)),
                        }.build(ctx, view),
                        
                        TimePicker {
                            hour: view.state.schedule_time.map(|(h, _)| h).unwrap_or(9),
                            minute: view.state.schedule_time.map(|(_, m)| m).unwrap_or(0),
                            on_change: Some(Arc::new(move |h, m| {
                                ActionEnvelope {
                                    id: time_id,
                                    payload: serde_json::to_vec(&SetScheduleTime(h, m)).unwrap(),
                                }
                            })),
                        }.build(ctx, view),
                    ]
                }.build(ctx, view),
                
                // Attachments
                FileUpload {
                    label: "Attach File".into(),
                    selected_file: view.state.compose_attachments.first().cloned(),
                    on_browse: None, 
                }.build(ctx, view),
                
                // Message
                FormControl {
                    id: None,
                    label: Some("Message".into()),
                    required: true,
                    error: None,
                    helper: Some("Markdown supported".into()),
                    child: Box::new(TextInput { 
                        value: view.state.compose_body.clone(),
                        placeholder: Some("Type your message...".into()), 
                        multiline: true,
                        width: Some(550.0),
                        height: Some(200.0),
                        ..Default::default() 
                    }.into_node()),
                }.build(ctx, view),
            ]
        }.into_node();

        Modal {
            id: WidgetNodeId::explicit("compose_modal"),
            title: "New Message".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleCompose, (|s: &mut InboxState, _: ToggleCompose, _| s.show_compose = false) as Handler<InboxState, ToggleCompose>)),
            width: Some(600.0),
            content: Box::new(
                Dropzone {
                    child: Box::new(content),
                    on_drop: Some(ctx.bind(FileSelected, (|s: &mut InboxState, _a: FileSelected, ctx| {
                        if let Some(paths) = ctx.input.as_drop_paths() {
                            s.compose_attachments.extend(paths.iter().cloned());
                        }
                    }) as Handler<InboxState, FileSelected>)),
                    on_drag_enter: None,
                    on_drag_leave: None,
                }.build(ctx, view)
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
