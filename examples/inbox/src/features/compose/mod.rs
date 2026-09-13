//! The compose modal: recipients, subject, schedule, attachments and message.

mod fields;

use crate::model::compose::{add_dropped_files, send_compose, set_compose_open};
use crate::model::{FileSelected, InboxState, SendCompose, SetComposeOpen};
use fields::{AttachmentField, MessageField, RecipientField, ScheduleFields, SubjectField};
use fission::core::ui::Widget;
use fission::core::{reduce_with, WidgetId};
use fission::widgets::{Dropzone, FocusScope, Modal, ModalAction, ModalMotion, VStack};

#[derive(Clone)]
pub struct ComposeModal;

impl From<ComposeModal> for Widget {
    fn from(_component: ComposeModal) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let modal_width =
            (view.viewport_size().width.max(0.0) - tokens.spacing.xxl).clamp(320.0, 760.0);
        let field_width = (modal_width - tokens.spacing.xxxl + tokens.spacing.s).max(240.0);
        let close = || ctx.bind(SetComposeOpen(false), reduce_with!(set_compose_open));

        Modal {
            id: WidgetId::explicit("compose_modal"),
            title: view.tr("compose.title"),
            is_open: true,
            on_dismiss: Some(close()),
            backdrop_semantics_identifier: Some("inbox.compose.backdrop".into()),
            close_semantics_identifier: Some("inbox.compose.close".into()),
            surface_semantics_identifier: Some("inbox.compose.surface".into()),
            width: Some(modal_width),
            content: FocusScope {
                id: None,
                is_barrier: true,
                children: vec![Dropzone {
                    id: None,
                    semantics_identifier: None,
                    child: VStack {
                        spacing: Some(tokens.spacing.s),
                        children: vec![
                            RecipientField { width: field_width }.into(),
                            SubjectField.into(),
                            ScheduleFields.into(),
                            AttachmentField.into(),
                            MessageField.into(),
                        ],
                    }
                    .into(),
                    active_child: None,
                    hover_child: None,
                    on_drop: Some(ctx.bind(FileSelected, reduce_with!(add_dropped_files))),
                    on_drag_enter: None,
                    on_drag_leave: None,
                }
                .into()],
            }
            .into(),
            actions: vec![
                ModalAction {
                    label: view.tr("compose.cancel"),
                    is_primary: false,
                    on_press: Some(close()),
                    semantics_identifier: Some("inbox.compose.cancel".into()),
                },
                ModalAction {
                    label: view.tr("compose.send"),
                    is_primary: true,
                    on_press: Some(ctx.bind(SendCompose, reduce_with!(send_compose))),
                    semantics_identifier: Some("inbox.compose.send".into()),
                },
            ],
            motion: Some(ModalMotion::Default),
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::model::{
        Email, EmailMessage, FileSelected, Folder, SendCompose, SetComposeBody, SetComposeOpen,
        SetComposeSubject, SetComposeTo, SetDatePickerOpen, SetScheduleDate, SetScheduleTime,
    };
    use anyhow::Result;
    use fission::core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
    use fission::core::Action;
    #[allow(unused_imports)]
    use fission::core::ActionEnvelope;
    use fission_test::TestHarness;
    #[allow(unused_imports)]
    use std::collections::HashSet;
    #[allow(unused_imports)]
    use std::sync::Arc;

    fn display_contains(h: &TestHarness<InboxState>, needle: &str) -> bool {
        h.get_last_display_list().is_some_and(|list| {
            list.ops.iter().any(|op| match op {
                fission::render::DisplayOp::DrawText { text, .. } => text.contains(needle),
                fission::render::DisplayOp::DrawRichText { runs, .. } => runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
                    .contains(needle),
                _ => false,
            })
        })
    }

    #[test]
    fn compose_multiline_trailing_newline_keeps_previous_line_visible() -> Result<()> {
        let mut h = TestHarness::new(InboxState::default()).with_root_widget(ComposeModal);
        h.pump()?;
        let body_node_id: fission_ir::WidgetId = WidgetId::explicit("compose_body_input");
        h.runtime
            .runtime_state
            .interaction
            .set_focused(Some(body_node_id));

        for ch in "first".chars() {
            h.send_event(InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Char(ch),
                modifiers: 0,
            }))?;
            h.pump()?;
        }
        assert!(display_contains(&h, "first"));

        h.send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Enter,
            modifiers: 0,
        }))?;
        h.pump()?;

        assert_eq!(
            h.runtime
                .get_app_state::<InboxState>()
                .expect("inbox state")
                .compose_body,
            "first\n"
        );
        assert!(
            display_contains(&h, "first"),
            "the committed first line must remain in the display list when the value ends in a newline"
        );
        assert!(
            h.runtime
                .runtime_state
                .scroll
                .offsets
                .values()
                .all(|offset| offset.abs() <= f32::EPSILON),
            "a trailing newline that still fits the field must not scroll the first line out of view: {:?}",
            h.runtime.runtime_state.scroll.offsets
        );
        Ok(())
    }

    #[test]
    fn compose_subject_and_body_accept_typing() -> Result<()> {
        let mut h = TestHarness::new(InboxState::default()).with_root_widget(ComposeModal);
        h.pump()?;

        let subject_node_id: fission_ir::WidgetId = WidgetId::explicit("compose_subject_input");
        let body_node_id: fission_ir::WidgetId = WidgetId::explicit("compose_body_input");

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
            pointer_id: Default::default(),
            kind: Default::default(),
            point: subject_center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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
            pointer_id: Default::default(),
            kind: Default::default(),
            point: body_center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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
            pointer_id: Default::default(),
            kind: Default::default(),
            point: center,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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
            pointer_id: Default::default(),
            kind: Default::default(),
            point: center2,
            button: PointerButton::Primary,
            modifiers: 0,
        }))?;
        h.send_event(InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
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
