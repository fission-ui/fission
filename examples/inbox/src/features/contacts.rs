use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use fission_core::ui::Node;
use fission_widgets::{Modal, ModalAction, DataTable, TableColumn, TableRow};
use crate::model::{InboxState, ToggleContacts};

pub struct ContactsModal;

impl Widget<InboxState> for ContactsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let data = vec![
            TableRow { id: "1".into(), cells: vec!["Alice".into(), "alice@example.com".into()] },
            TableRow { id: "2".into(), cells: vec!["Bob".into(), "bob@example.com".into()] },
            TableRow { id: "3".into(), cells: vec!["Charlie".into(), "charlie@example.com".into()] },
        ];
        
        Modal {
            id: WidgetNodeId::explicit("contacts_modal"),
            title: "Contacts".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleContacts, |s, _| s.show_contacts = false)),
            width: Some(500.0),
            content: Box::new(
                DataTable {
                    id: WidgetNodeId::explicit("contacts_table"),
                    columns: vec![
                        TableColumn { id: "name".into(), title: "Name".into(), width: 150.0, sortable: true },
                        TableColumn { id: "email".into(), title: "Email".into(), width: 250.0, sortable: true },
                    ],
                    rows: data,
                    selected_ids: vec![],
                    on_selection_change: None,
                }.build(ctx, view)
            ),
            actions: vec![
                ModalAction { label: "Done".into(), is_primary: true, on_press: Some(ctx.bind(ToggleContacts, |s, _| s.show_contacts = false)) }
            ]
        }.build(ctx, view)
    }
}
