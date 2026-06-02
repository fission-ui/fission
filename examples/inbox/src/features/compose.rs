use crate::model::{
    Email, EmailMessage, FileSelected, Folder, InboxState, SendCompose, SetComposeBody,
    SetComposeOpen, SetComposeSubject, SetComposeTo, SetDatePickerOpen, SetScheduleDate,
    SetScheduleTime,
};
use chrono::Local;
use fission::core::ui::Widget;
use fission::core::{reduce_with, ActionEnvelope, WidgetId};
use fission::widgets::{
    Combobox, DatePicker, Dropzone, FileUpload, FocusScope, FormControl, Modal, ModalAction,
    TextInput, TimePicker, VStack, Wrap,
};
use serde_json;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeModal;

impl From<ComposeModal> for Widget {
    fn from(_component: ComposeModal) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let viewport_width = view.viewport_size().width.max(0.0);
        let modal_width = (viewport_width - 48.0).clamp(320.0, 760.0);
        let field_width = (modal_width - 56.0).max(240.0);

        // Register Handlers
        let to_id = ctx
            .bind(
                SetComposeTo("".into()),
                reduce_with!((|s: &mut InboxState, a: SetComposeTo, _| s.compose_to = a.0)),
            )
            .id;
        let subject_id = ctx
            .bind(
                SetComposeSubject("".into()),
                reduce_with!(
                    (|s: &mut InboxState, a: SetComposeSubject, _| s.compose_subject = a.0)
                ),
            )
            .id;
        let body_id = ctx
            .bind(
                SetComposeBody("".into()),
                reduce_with!((|s: &mut InboxState, a: SetComposeBody, _| s.compose_body = a.0)),
            )
            .id;
        let date_id = ctx
            .bind(
                SetScheduleDate(chrono::Local::now().date_naive()),
                reduce_with!(
                    (|s: &mut InboxState, a: SetScheduleDate, _| {
                        s.schedule_date = Some(a.0);
                        s.is_date_picker_open = false;
                    })
                ),
            )
            .id;
        let time_id = ctx
            .bind(
                SetScheduleTime(0, 0),
                reduce_with!(
                    (|s: &mut InboxState, a: SetScheduleTime, _| s.schedule_time =
                        Some((a.0, a.1)))
                ),
            )
            .id;
        let date_picker_open_id = ctx
            .bind(
                SetDatePickerOpen(false),
                reduce_with!(
                    (|s: &mut InboxState, a: SetDatePickerOpen, _| s.is_date_picker_open = a.0)
                ),
            )
            .id;
        let send_id = ctx
            .bind(
                SendCompose,
                reduce_with!(
                    (|s: &mut InboxState, _: SendCompose, _| {
                        let subject = if s.compose_subject.trim().is_empty() {
                            "(no subject)".to_string()
                        } else {
                            s.compose_subject.trim().to_string()
                        };
                        let body = if s.compose_body.trim().is_empty() {
                            "(empty message)".to_string()
                        } else {
                            s.compose_body.trim().to_string()
                        };
                        let to: Vec<String> = s
                            .compose_to
                            .split(',')
                            .map(|v| v.trim().to_string())
                            .filter(|v| !v.is_empty())
                            .collect();

                        let msg_id = s.next_message_id;
                        s.next_message_id += 1;
                        let thread_id = s.next_email_id;
                        s.next_email_id += 1;

                        let message = EmailMessage {
                            id: msg_id,
                            from: "You".into(),
                            to: if to.is_empty() {
                                vec!["team@fission.rs".into()]
                            } else {
                                to
                            },
                            cc: Vec::new(),
                            body,
                            sent_at: Local::now().naive_local(),
                        };

                        let mut folders = HashSet::new();
                        folders.insert(Folder::Sent);

                        let mut email = Email {
                            id: thread_id,
                            subject,
                            sender: "You".into(),
                            preview: String::new(),
                            folders,
                            is_read: true,
                            is_flagged: false,
                            labels: vec!["Sent".into()],
                            messages: vec![message],
                        };
                        email.refresh_preview();
                        s.emails.insert(0, email);

                        s.compose_to.clear();
                        s.compose_subject.clear();
                        s.compose_body.clear();
                        s.compose_attachments.clear();
                        s.schedule_date = None;
                        s.schedule_time = None;
                        s.is_date_picker_open = false;

                        s.show_compose = false;
                        s.show_toast = true;
                        s.toast_message = Some("Message sent".into());
                    })
                ),
            )
            .id;

        let subject_node_id = WidgetId::explicit("compose_subject_input");
        let body_node_id = WidgetId::explicit("compose_body_input");
        let toggle_date_picker = ActionEnvelope {
            id: date_picker_open_id,
            payload: serde_json::to_vec(&SetDatePickerOpen(!view.state().is_date_picker_open))
                .unwrap(),
        };
        let close_date_picker = ActionEnvelope {
            id: date_picker_open_id,
            payload: serde_json::to_vec(&SetDatePickerOpen(false)).unwrap(),
        };

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
                    child: {
                        let all_recipients = vec![
                            "alice@example.com".to_string(),
                            "bob@example.com".to_string(),
                            "team@fission.rs".to_string(),
                        ];
                        let query = view.state().compose_to.trim().to_lowercase();
                        let suggestions: Vec<String> = if query.is_empty() {
                            Vec::new()
                        } else {
                            all_recipients
                                .iter()
                                .filter(|item| item.to_lowercase().contains(&query))
                                .cloned()
                                .collect()
                        };
                        let has_exact_match = all_recipients
                            .iter()
                            .any(|item| item.eq_ignore_ascii_case(view.state().compose_to.trim()));

                        Combobox {
                            id: WidgetId::explicit("compose_to"),
                            value: view.state().compose_to.clone(),
                            items: suggestions,
                            is_open: !query.is_empty() && !has_exact_match,
                            width: Some(field_width),
                            max_popup_height: Some(180.0),
                            on_change: Some(ActionEnvelope {
                                id: to_id,
                                payload: Vec::new(),
                            }),
                            on_select: Some(Arc::new(move |val| ActionEnvelope {
                                id: to_id,
                                payload: serde_json::to_vec(&SetComposeTo(val)).unwrap(),
                            })),
                            on_toggle: None,
                        }
                        .into()
                    },
                }
                .into(),
                // Subject
                FormControl {
                    id: None,
                    label: Some("Subject".into()),
                    required: false,
                    error: None,
                    helper: None,
                    child: TextInput {
                        id: Some(subject_node_id),
                        value: view.state().compose_subject.clone(),
                        placeholder: Some("Subject".into()),
                        on_change: Some(ActionEnvelope {
                            id: subject_id,
                            payload: Vec::new(),
                        }),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                // Schedule
                Wrap {
                    direction: fission::op::FlexDirection::Row,
                    spacing: Some(12.0),
                    children: vec![
                        DatePicker {
                            id: WidgetId::explicit("schedule_date"),
                            value: view.state().schedule_date,
                            is_open: view.state().is_date_picker_open,
                            width: None,
                            on_change: Some(Arc::new(move |d| ActionEnvelope {
                                id: date_id,
                                payload: serde_json::to_vec(&SetScheduleDate(d)).unwrap(),
                            })),
                            on_toggle: Some(toggle_date_picker.clone()),
                            on_close: Some(close_date_picker.clone()),
                        }
                        .into(),
                        TimePicker {
                            hour: view.state().schedule_time.map(|(h, _)| h).unwrap_or(9),
                            minute: view.state().schedule_time.map(|(_, m)| m).unwrap_or(0),
                            on_change: Some(Arc::new(move |h, m| ActionEnvelope {
                                id: time_id,
                                payload: serde_json::to_vec(&SetScheduleTime(h, m)).unwrap(),
                            })),
                        }
                        .into(),
                    ],
                }
                .into(),
                // Attachments
                FileUpload {
                    label: "Attach File".into(),
                    selected_file: view.state().compose_attachments.first().cloned(),
                    on_browse: None,
                }
                .into(),
                // Message
                FormControl {
                    id: None,
                    label: Some("Message".into()),
                    required: true,
                    error: None,
                    helper: Some("Markdown supported".into()),
                    child: TextInput {
                        id: Some(body_node_id),
                        value: view.state().compose_body.clone(),
                        placeholder: Some("Type your message...".into()),
                        on_change: Some(ActionEnvelope {
                            id: body_id,
                            payload: Vec::new(),
                        }),
                        multiline: true,
                        height: Some(160.0),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
            ],
        }
        .into();

        Modal {
            id: WidgetId::explicit("compose_modal"),
            title: "New Message".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(
                SetComposeOpen(false),
                reduce_with!((|s: &mut InboxState, a: SetComposeOpen, _| s.show_compose = a.0)),
            )),
            width: Some(modal_width),
            content: FocusScope {
                id: None,
                is_barrier: true,
                children: vec![Dropzone {
                    child: content,
                    on_drop: Some(ctx.bind(
                        FileSelected,
                        reduce_with!(
                            (|s: &mut InboxState, _a: FileSelected, ctx| {
                                if let Some(paths) = ctx.input.as_drop_paths() {
                                    s.compose_attachments.extend(paths.iter().cloned());
                                }
                            })
                        ),
                    )),
                    on_drag_enter: None,
                    on_drag_leave: None,
                }
                .into()],
            }
            .into(),
            actions: vec![
                ModalAction {
                    label: "Cancel".into(),
                    is_primary: false,
                    on_press: Some(ctx.bind(
                        SetComposeOpen(false),
                        reduce_with!(
                            (|s: &mut InboxState, a: SetComposeOpen, _| s.show_compose = a.0)
                        ),
                    )),
                },
                ModalAction {
                    label: "Send".into(),
                    is_primary: true,
                    on_press: Some(ActionEnvelope {
                        id: send_id,
                        payload: serde_json::to_vec(&SendCompose).unwrap(),
                    }),
                },
            ],
        }
        .into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use fission::core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
    use fission::core::Action;
    use fission_test::TestHarness;

    #[test]
    fn compose_subject_and_body_accept_typing() -> Result<()> {
        let mut h = TestHarness::new(InboxState::default()).with_root_widget(ComposeModal);
        h.pump()?;

        let subject_node_id: fission_ir::WidgetId =
            WidgetId::explicit("compose_subject_input").into();
        let body_node_id: fission_ir::WidgetId = WidgetId::explicit("compose_body_input").into();

        let subject_rect = h
            .last_snapshot
            .as_ref()
            .unwrap()
            .get_node_rect(subject_node_id)
            .expect("subject rect");
        let subject_center = fission::core::LayoutPoint::new(
            subject_rect.x() + subject_rect.width() / 2.0,
            subject_rect.y() + subject_rect.height() / 2.0,
        );

        h.send_event(InputEvent::Pointer(PointerEvent::Down {
            point: subject_center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            point: subject_center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        assert_eq!(
            h.runtime.runtime_state.interaction.focused,
            Some(subject_node_id),
            "subject should be focused after clicking it"
        );
        h.send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Char('a'),
            modifiers: 0,
        }))?;
        h.pump()?;

        let state = h.runtime.get_app_state::<InboxState>().unwrap();
        assert_eq!(state.compose_subject, "a");

        let body_rect = h
            .last_snapshot
            .as_ref()
            .unwrap()
            .get_node_rect(body_node_id)
            .expect("body rect");
        let body_center = fission::core::LayoutPoint::new(
            body_rect.x() + body_rect.width() / 2.0,
            body_rect.y() + body_rect.height() / 2.0,
        );

        h.send_event(InputEvent::Pointer(PointerEvent::Down {
            point: body_center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            point: body_center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        if h.runtime.runtime_state.interaction.focused != Some(body_node_id) {
            let focused = h.runtime.runtime_state.interaction.focused;
            let ir = h.last_ir.as_ref().unwrap();
            let (role, value) = focused
                .and_then(|id| ir.nodes.get(&id))
                .and_then(|n| match &n.op {
                    fission_ir::Op::Semantics(s) => Some((s.role, s.value.clone())),
                    _ => None,
                })
                .unwrap_or((fission::Role::Generic, None));
            let snap = h.last_snapshot.as_ref().unwrap();
            let focused_rect = focused.and_then(|id| snap.get_node_rect(id));
            let body_rect_now = snap.get_node_rect(body_node_id);
            panic!(
                "body should be focused after clicking it; focused={:?} role={:?} value={:?} focused_rect={:?} body_rect={:?}",
                focused, role, value, focused_rect, body_rect_now
            );
        }
        h.send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Char('b'),
            modifiers: 0,
        }))?;
        h.pump()?;

        let state = h.runtime.get_app_state::<InboxState>().unwrap();
        assert_eq!(
            state.compose_subject, "a",
            "typing in body should not affect subject"
        );
        assert_eq!(state.compose_body, "b");

        Ok(())
    }

    #[test]
    fn compose_date_picker_opens_and_selecting_date_closes() -> Result<()> {
        let mut h = TestHarness::new(InboxState::default()).with_root_widget(ComposeModal);
        h.pump()?;

        let ir = h.last_ir.as_ref().unwrap();

        // Find the date-picker toggle button by action id (SetDatePickerOpen).
        let toggle_action_id = SetDatePickerOpen::static_id().as_u128();
        let toggle_node = ir
            .nodes
            .iter()
            .find_map(|(id, n)| {
                if let fission_ir::Op::Semantics(s) = &n.op {
                    if s.actions
                        .entries
                        .iter()
                        .any(|e| e.action_id == toggle_action_id)
                    {
                        return Some(*id);
                    }
                }
                None
            })
            .expect("toggle datepicker node");

        let rect = h
            .last_snapshot
            .as_ref()
            .unwrap()
            .get_node_rect(toggle_node)
            .unwrap();
        let center = fission::core::LayoutPoint::new(
            rect.x() + rect.width() / 2.0,
            rect.y() + rect.height() / 2.0,
        );

        h.send_event(InputEvent::Pointer(PointerEvent::Down {
            point: center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            point: center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.pump()?;
        assert!(
            h.runtime
                .get_app_state::<InboxState>()
                .unwrap()
                .is_date_picker_open
        );

        // Find any calendar day button by action id (SetScheduleDate).
        let ir2 = h.last_ir.as_ref().unwrap();
        let date_action_id = SetScheduleDate::static_id().as_u128();
        let day_node = ir2
            .nodes
            .iter()
            .find_map(|(id, n)| {
                if let fission_ir::Op::Semantics(s) = &n.op {
                    if s.actions
                        .entries
                        .iter()
                        .any(|e| e.action_id == date_action_id)
                    {
                        return Some(*id);
                    }
                }
                None
            })
            .expect("calendar day node");

        let rect2 = h
            .last_snapshot
            .as_ref()
            .unwrap()
            .get_node_rect(day_node)
            .unwrap();
        let center2 = fission::core::LayoutPoint::new(
            rect2.x() + rect2.width() / 2.0,
            rect2.y() + rect2.height() / 2.0,
        );

        // Sanity check: the hit-test at this point should see a Default-trigger action
        // for SetScheduleDate somewhere up the ancestor chain (otherwise the click
        // will dismiss via backdrop or no-op).
        let mut hits_date_action = false;
        let hit = fission::core::hit_test::hit_test_with_scroll(
            ir2,
            h.last_snapshot.as_ref().unwrap(),
            &h.runtime.runtime_state.scroll,
            center2,
        );
        if let Some(hit) = hit {
            let mut cur = Some(hit);
            while let Some(id) = cur {
                if let Some(n) = ir2.nodes.get(&id) {
                    if let fission_ir::Op::Semantics(s) = &n.op {
                        if s.actions
                            .entries
                            .iter()
                            .any(|e| e.action_id == date_action_id)
                        {
                            hits_date_action = true;
                            break;
                        }
                    }
                    cur = n.parent;
                } else {
                    break;
                }
            }
        }
        if !hits_date_action {
            // Find a descendant paint node for the day button and report its rect for debugging.
            let snap = h.last_snapshot.as_ref().unwrap();
            let mut q = vec![day_node];
            let mut day_desc_paint_rect = None;
            let mut day_desc_drawrect_rect = None;
            while let Some(id) = q.pop() {
                if let Some(n) = ir2.nodes.get(&id) {
                    if let fission_ir::Op::Paint(_) = n.op {
                        if day_desc_paint_rect.is_none() {
                            day_desc_paint_rect = snap.get_node_rect(id);
                        }
                        if matches!(
                            n.op,
                            fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect { .. })
                        ) {
                            day_desc_drawrect_rect = snap.get_node_rect(id);
                            break;
                        }
                    }
                    for c in &n.children {
                        q.push(*c);
                    }
                }
            }

            let hit_sem_role = hit
                .and_then(|hid| ir2.nodes.get(&hid))
                .and_then(|n| match &n.op {
                    fission_ir::Op::Semantics(s) => Some(s.role),
                    _ => None,
                });
            let hit_op = hit.and_then(|hid| ir2.nodes.get(&hid)).map(|n| &n.op);
            let hit_rect = hit.and_then(|hid| snap.get_node_rect(hid));

            panic!(
                "expected click point to hit a SetScheduleDate action; day_node={:?} day_rect={:?} day_desc_paint_rect={:?} day_desc_drawrect_rect={:?} hit={:?} hit_rect={:?} hit_op={:?} hit_sem_role={:?}",
                day_node,
                rect2,
                day_desc_paint_rect,
                day_desc_drawrect_rect,
                hit,
                hit_rect,
                hit_op,
                hit_sem_role
            );
        }

        h.send_event(InputEvent::Pointer(PointerEvent::Down {
            point: center2,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            point: center2,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.pump()?;

        let state = h.runtime.get_app_state::<InboxState>().unwrap();
        assert!(
            state.schedule_date.is_some(),
            "schedule_date should be set after selecting a day (is_open={})",
            state.is_date_picker_open
        );
        assert!(
            !state.is_date_picker_open,
            "date picker should close after selecting a day"
        );

        Ok(())
    }
}
