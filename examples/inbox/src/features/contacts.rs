use crate::model::{InboxState, SetContactsOpen, ToggleContactSelection};
use fission_core::ui::Node;
use fission_core::{BuildCtx, Handler, View, Widget, WidgetNodeId};
use fission_widgets::{DataTable, Modal, ModalAction, TableColumn, TableRow};
use serde_json;
use std::sync::Arc;

pub struct ContactsModal;

impl Widget<InboxState> for ContactsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let viewport_width = view.viewport_size().width.max(0.0);
        let toggle_id = ctx
            .bind(
                ToggleContactSelection("".into()),
                (|s: &mut InboxState, a: ToggleContactSelection, _| {
                    if let Some(pos) = s.contact_selected_ids.iter().position(|id| id == &a.0) {
                        s.contact_selected_ids.remove(pos);
                    } else {
                        s.contact_selected_ids.push(a.0);
                    }
                }) as Handler<InboxState, ToggleContactSelection>,
            )
            .id;
        let data = vec![
            TableRow {
                id: "1".into(),
                cells: vec!["Alice".into(), "alice@example.com".into()],
            },
            TableRow {
                id: "2".into(),
                cells: vec!["Bob".into(), "bob@example.com".into()],
            },
            TableRow {
                id: "3".into(),
                cells: vec!["Charlie".into(), "charlie@example.com".into()],
            },
        ];

        Modal {
            id: WidgetNodeId::explicit("contacts_modal"),
            title: "Contacts".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(
                SetContactsOpen(false),
                (|s, a, _| s.show_contacts = a.0) as Handler<InboxState, SetContactsOpen>,
            )),
            width: Some((viewport_width - 48.0).clamp(320.0, 560.0)),
            content: Box::new(
                DataTable {
                    id: WidgetNodeId::explicit("contacts_table"),
                    columns: vec![
                        TableColumn {
                            id: "name".into(),
                            title: "Name".into(),
                            width: 150.0,
                            sortable: true,
                        },
                        TableColumn {
                            id: "email".into(),
                            title: "Email".into(),
                            width: 250.0,
                            sortable: true,
                        },
                    ],
                    rows: data,
                    selected_ids: view.state.contact_selected_ids.clone(),
                    on_selection_change: Some(Arc::new(move |row_id| {
                        fission_core::ActionEnvelope {
                            id: toggle_id,
                            payload: serde_json::to_vec(&ToggleContactSelection(row_id)).unwrap(),
                        }
                    })),
                }
                .build(ctx, view),
            ),
            actions: vec![ModalAction {
                label: "Done".into(),
                is_primary: true,
                on_press: Some(ctx.bind(
                    SetContactsOpen(false),
                    (|s, a, _| s.show_contacts = a.0) as Handler<InboxState, SetContactsOpen>,
                )),
            }],
        }
        .build(ctx, view)
    }
}
