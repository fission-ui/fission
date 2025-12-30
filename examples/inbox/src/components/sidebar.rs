use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use fission_core::ui::{Container, Node, Text, TextContent, Button, ButtonVariant};
use fission_core::op::Color;
use fission_widgets::{VStack, HStack, Tooltip, Switch, Slider, ProgressBar, TreeView, TreeItem, Divider};
use fission_icons::material;
use crate::model::{InboxState, SelectFolder, ToggleNotifications, SetStorageUsage, ToggleSettings, ToggleContacts, Navigate, ToggleFolderExpand};

pub struct Sidebar;

impl Widget<InboxState> for Sidebar {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Container::new(
            VStack {
                spacing: Some(16.0),
                children: vec![
                    Text::new("FISSION MAIL").size(18.0).into_node(),
                    
                    // Folder Tree
                    TreeView {
                        items: vec![
                            TreeItem { 
                                id: "inbox".into(), 
                                label: "Inbox".into(), 
                                icon: Some(material::content::mail::regular().into()), 
                                children: vec![], 
                                on_toggle: None, 
                                on_select: Some(ctx.bind(Navigate("/inbox".into()), |s, a| s.current_path = a.0)) 
                            },
                            TreeItem { 
                                id: "starred".into(), 
                                label: "Starred".into(), 
                                icon: Some(material::toggle::star::regular().into()), 
                                children: vec![], 
                                on_toggle: None, 
                                on_select: Some(ctx.bind(Navigate("/starred".into()), |s, a| s.current_path = a.0)) 
                            },
                            TreeItem { 
                                id: "sent".into(), 
                                label: "Sent".into(), 
                                icon: Some(material::content::send::regular().into()), 
                                children: vec![], 
                                on_toggle: None, 
                                on_select: Some(ctx.bind(Navigate("/sent".into()), |s, a| s.current_path = a.0)) 
                            },
                            TreeItem { 
                                id: "folders".into(), 
                                label: "Folders".into(), 
                                icon: Some(material::file::folder::regular().into()), 
                                on_toggle: Some(ctx.bind(ToggleFolderExpand("folders".into()), |s, a| { 
                                    if s.expanded_folders.contains(&a.0) { s.expanded_folders.remove(&a.0); } else { s.expanded_folders.insert(a.0); } 
                                })),
                                on_select: None,
                                children: vec![
                                    TreeItem { id: "work".into(), label: "Work".into(), icon: None, children: vec![], on_toggle: None, on_select: Some(ctx.bind(Navigate("/work".into()), |s, a| s.current_path = a.0)) },
                                    TreeItem { id: "personal".into(), label: "Personal".into(), icon: None, children: vec![], on_toggle: None, on_select: Some(ctx.bind(Navigate("/personal".into()), |s, a| s.current_path = a.0)) },
                                ]
                            },
                        ],
                        expanded_ids: view.state.expanded_folders.clone(),
                        selected_id: Some(view.state.current_path.trim_start_matches('/').to_string()),
                    }.build(ctx, view),
                    
                    fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                    
                    Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view),
                    
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new("Contacts").into_node())),
                        on_press: Some(ctx.bind(ToggleContacts, |s, _| s.show_contacts = true)),
                        ..Default::default()
                    }.into_node(),
                    
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new("Settings").into_node())),
                        on_press: Some(ctx.bind(ToggleSettings, |s, _| s.show_settings = true)),
                        ..Default::default()
                    }.into_node(),
                ],
            }.build(ctx, view)
        )
        .bg(Color { r: 245, g: 245, b: 247, a: 255 })
        .padding_all(16.0)
        .into_node()
    }
}