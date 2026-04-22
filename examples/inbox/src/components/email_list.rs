use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler};
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Checkbox, Container, Node, Row, Text, TextContent};
use fission_widgets::{VStack, HStack, LazyColumn, Tabs, TabItem, TextInput, MenuButton, MenuItem, Badge, Divider, Icon, SegmentedControl, Pagination, EmptyState, Hero, DropDown, Tooltip, Popover, DateRangePicker, RangeSlider, Wrap, Tag};
use crate::model::{InboxState, Folder, SelectTab, UpdateSearch, ToggleFilterDropdown, ToggleEmailSelection, ToggleFlag, SetComposeOpen, Navigate, SetMobileMenuOpen, SetFilterMode, SetPage, SetAdvancedFiltersOpen, SetSortOption, SetHelpPopoverOpen};
use fission_icons::material;
use std::sync::Arc;
use serde_json;
use fission_core::ActionEnvelope;

pub struct EmailList {
    pub folder: String,
}

fn folder_from_route(route: &str) -> Folder {
    match route.to_lowercase().as_str() {
        "inbox" => Folder::Inbox,
        "starred" => Folder::Starred,
        "sent" => Folder::Sent,
        "drafts" => Folder::Drafts,
        "trash" => Folder::Trash,
        other => Folder::Custom(other.to_string()),
    }
}

impl Widget<InboxState> for EmailList {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let t = |key: &str| {
            view.env
                .i18n
                .get(&view.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let mut list_items = vec![];

        let folder = folder_from_route(&self.folder);
        let folder_label = match &folder {
            Folder::Inbox => view.env.i18n.get(&view.env.locale, "folder.inbox").map(|s| s.to_string()).unwrap_or_else(|| "Inbox".into()),
            Folder::Starred => view.env.i18n.get(&view.env.locale, "folder.starred").map(|s| s.to_string()).unwrap_or_else(|| "Starred".into()),
            Folder::Sent => view.env.i18n.get(&view.env.locale, "folder.sent").map(|s| s.to_string()).unwrap_or_else(|| "Sent".into()),
            Folder::Drafts => view.env.i18n.get(&view.env.locale, "folder.drafts").map(|s| s.to_string()).unwrap_or_else(|| "Drafts".into()),
            Folder::Trash => view.env.i18n.get(&view.env.locale, "folder.trash").map(|s| s.to_string()).unwrap_or_else(|| "Trash".into()),
            Folder::Custom(label) => label.clone(),
        };
        let folder_path = match &folder {
            Folder::Inbox => "inbox".to_string(),
            Folder::Starred => "starred".to_string(),
            Folder::Sent => "sent".to_string(),
            Folder::Drafts => "drafts".to_string(),
            Folder::Trash => "trash".to_string(),
            Folder::Custom(label) => label.to_lowercase(),
        };
        let unread_count = view
            .state
            .emails
            .iter()
            .filter(|e| e.folders.contains(&folder) && !e.is_read)
            .count();
        
        // Register handlers and get IDs
        let filter_id = ctx.bind(SetFilterMode(0), (|s: &mut InboxState, a: SetFilterMode, _| s.filter_mode = a.0) as Handler<InboxState, SetFilterMode>).id;
        let page_id = ctx.bind(SetPage(0), (|s: &mut InboxState, a: SetPage, _| s.page = a.0) as Handler<InboxState, SetPage>).id;
        let filters_open_id = ctx.bind(SetAdvancedFiltersOpen(false), (|s: &mut InboxState, a: SetAdvancedFiltersOpen, _| s.show_advanced_filters = a.0) as Handler<InboxState, SetAdvancedFiltersOpen>).id;
        let sort_id = ctx.bind(SetSortOption("Newest".into()), (|s: &mut InboxState, a: SetSortOption, _| s.sort_option = a.0) as Handler<InboxState, SetSortOption>).id;
        let search_id = ctx.bind(UpdateSearch("".into()), (|s: &mut InboxState, a: UpdateSearch, _| s.search_query = a.0) as Handler<InboxState, UpdateSearch>).id;
        let select_id = ctx.bind(ToggleEmailSelection(0), (|s: &mut InboxState, a: ToggleEmailSelection, _| {
            if let Some(pos) = s.selected_emails.iter().position(|id| *id == a.0) {
                s.selected_emails.remove(pos);
            } else {
                s.selected_emails.push(a.0);
            }
        }) as Handler<InboxState, ToggleEmailSelection>).id;
        let flag_id = ctx.bind(ToggleFlag(0), (|s: &mut InboxState, a: ToggleFlag, _| {
            if let Some(email) = s.emails.iter_mut().find(|e| e.id == a.0) {
                email.is_flagged = !email.is_flagged;
                if email.is_flagged {
                    email.folders.insert(Folder::Starred);
                } else {
                    email.folders.remove(&Folder::Starred);
                }
            }
        }) as Handler<InboxState, ToggleFlag>).id;
        let navigate_id = ctx.bind(Navigate("".into()), (|s: &mut InboxState, a: Navigate, _| {
            s.navigate_to(a.0);
            s.show_mobile_menu = false;
            if let Some(id) = s.selected_email_id {
                if let Some(email) = s.emails.iter_mut().find(|e| e.id == id) {
                    email.is_read = true;
                }
            }
        }) as Handler<InboxState, Navigate>).id;
        let tab_id = ctx.bind(SelectTab(0), (|s: &mut InboxState, a: SelectTab, _| s.active_tab = a.0) as Handler<InboxState, SelectTab>).id;
        let menu_toggle = ctx.bind(ToggleFilterDropdown, (|s: &mut InboxState, _: ToggleFilterDropdown, _| s.show_filter_dropdown = !s.show_filter_dropdown) as Handler<InboxState, ToggleFilterDropdown>);
        
        // Header
        list_items.push(
            Row {
                gap: Some(6.0),
                children: vec![
                    Text::new(folder_label).size(20.0).into_node(),
                    Badge { text: format!("{} {}", unread_count, t("badge.new")), ..Default::default() }.build(ctx, view),
                ],
                ..Default::default()
            }.into_node()
        );
        
        // Filter + Search row
        let sort_toggle = if view.state.sort_option == "Newest" { "Oldest" } else { "Newest" };
        let sort_toggle = ActionEnvelope {
            id: sort_id,
            payload: serde_json::to_vec(&SetSortOption(sort_toggle.into())).unwrap(),
        };
        // Search row
        list_items.push(
            TextInput {
                value: view.state.search_query.clone(),
                placeholder: Some(TextContent::Key("search.placeholder".into())),
                on_change: Some(ActionEnvelope { id: search_id, payload: Vec::new() }),
                ..Default::default()
            }.into_node(),
        );

        // Filter row
        list_items.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    SegmentedControl {
                        options: vec!["All".into(), "Unread".into(), "Starred".into()],
                        selected_index: view.state.filter_mode,
                        on_change: Some(Arc::new(move |idx| {
                            ActionEnvelope {
                                id: filter_id,
                                payload: serde_json::to_vec(&SetFilterMode(idx)).unwrap(),
                            }
                        })),
                    }.build(ctx, view),
                    fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                    DropDown {
                        selected: Some(view.state.sort_option.clone()),
                        options: vec!["Newest".into(), "Oldest".into(), "Unread".into()],
                        on_toggle: Some(sort_toggle),
                        ..Default::default()
                    }.build(ctx, view),
                    Popover {
                        id: WidgetNodeId::explicit("advanced_filters"),
                        is_open: view.state.show_advanced_filters,
                        on_toggle: Some(ActionEnvelope {
                            id: filters_open_id,
                            payload: serde_json::to_vec(&SetAdvancedFiltersOpen(!view.state.show_advanced_filters)).unwrap(),
                        }),
                        on_close: Some(ActionEnvelope {
                            id: filters_open_id,
                            payload: serde_json::to_vec(&SetAdvancedFiltersOpen(false)).unwrap(),
                        }),
                        trigger: Box::new(
                            Button {
                                variant: ButtonVariant::Outline,
                                child: Some(Box::new(HStack {
                                    spacing: Some(6.0),
                                    children: vec![
                                        Icon::svg(material::content::filter_list::regular()).size(18.0).into_node(),
                                        Text::new(TextContent::Key("header.filters".into())).into_node(),
                                    ],
                                }.into_node())),
                                on_press: Some(ActionEnvelope {
                                    id: filters_open_id,
                                    payload: serde_json::to_vec(&SetAdvancedFiltersOpen(!view.state.show_advanced_filters)).unwrap(),
                                }),
                                ..Default::default()
                            }.into_node()
                        ),
                        content: Box::new(
                            Container::new(
                                VStack {
                                    spacing: Some(12.0),
                                    children: vec![
                                        Text::new(TextContent::Key("filter.date_range".into())).size(12.0).into_node(),
                                        DateRangePicker {
                                            id_start: WidgetNodeId::explicit("filter_date_start"),
                                            id_end: WidgetNodeId::explicit("filter_date_end"),
                                            start: view.state.schedule_date,
                                            end: view.state.schedule_date,
                                            is_start_open: false,
                                            is_end_open: false,
                                            on_change: None,
                                            on_toggle_start: None,
                                            on_toggle_end: None,
                                            on_close_start: None,
                                            on_close_end: None,
                                        }.build(ctx, view),
                                        Text::new(TextContent::Key("filter.size_mb".into())).size(12.0).into_node(),
                                        RangeSlider {
                                            id: None,
                                            start: 5.0,
                                            end: 50.0,
                                            min: 0.0,
                                            max: 100.0,
                                            on_change: None,
                                        }.build(ctx, view),
                                    ],
                                }.into_node()
                            )
                            .padding_all(12.0)
                            .width(260.0)
                            .into_node()
                        ),
                    }.build(ctx, view),
                ]
            }.build(ctx, view)
        );

        list_items.push(
            Tabs {
                active_index: view.state.active_tab,
                items: vec![
                    TabItem {
                        title: t("tabs.primary"),
                        content: fission_core::ui::widgets::Spacer::default().into_node(),
                        on_press: Some(ActionEnvelope {
                            id: tab_id,
                            payload: serde_json::to_vec(&SelectTab(0)).unwrap(),
                        }),
                    },
                    TabItem {
                        title: t("tabs.social"),
                        content: fission_core::ui::widgets::Spacer::default().into_node(),
                        on_press: Some(ActionEnvelope {
                            id: tab_id,
                            payload: serde_json::to_vec(&SelectTab(1)).unwrap(),
                        }),
                    },
                    TabItem {
                        title: t("tabs.promotions"),
                        content: fission_core::ui::widgets::Spacer::default().into_node(),
                        on_press: Some(ActionEnvelope {
                            id: tab_id,
                            payload: serde_json::to_vec(&SelectTab(2)).unwrap(),
                        }),
                    },
                ],
            }.build(ctx, view)
        );

        let mut emails: Vec<_> = view
            .state
            .emails
            .iter()
            .filter(|e| e.folders.contains(&folder))
            .collect();

        if !view.state.search_query.trim().is_empty() {
            emails.retain(|e| e.matches_query(&view.state.search_query));
        }

        match view.state.filter_mode {
            0 => {}
            1 => emails.retain(|e| !e.is_read),
            2 => emails.retain(|e| e.is_flagged),
            _ => {}
        }

        match view.state.sort_option.as_str() {
            "Oldest" => emails.sort_by_key(|e| e.last_message().sent_at),
            "Unread" => emails.sort_by_key(|e| e.is_read),
            _ => emails.sort_by_key(|e| std::cmp::Reverse(e.last_message().sent_at)),
        }

        let page_size = 8usize;
        let total_pages = ((emails.len() + page_size - 1) / page_size).max(1);
        let current_page = view.state.page.max(1).min(total_pages);
        let start_idx = (current_page - 1) * page_size;
        let end_idx = (start_idx + page_size).min(emails.len());

        if emails.is_empty() {
            list_items.push(
                EmptyState {
                                icon: Some(Box::new(Icon::svg(material::content::inbox::regular()).size(48.0).color(tokens.colors.text_primary).into_node())),
                                title: t("empty.no_emails"),
                                description: Some(t("empty.caught_up")),
                                action: Some(Box::new(
                                    Button {
                                        child: Some(Box::new(Text::new(t("action.refresh")).into_node())),
                                        on_press: None,
                                        ..Default::default()
                                    }.into_node()
                                )),
                            }.build(ctx, view)
                        );
        } else {
            let mut email_nodes = Vec::new();
            for (idx, email) in emails[start_idx..end_idx].iter().enumerate() {
                let path = format!("/{}/{}", folder_path, email.id);
                let is_selected = view.state.selected_emails.contains(&email.id);
                let star_icon = if email.is_flagged {
                    material::toggle::star::regular()
                } else {
                    material::toggle::star_border::regular()
                };
                let subject_color = if email.is_read {
                    tokens.colors.text_secondary
                } else {
                    tokens.colors.text_primary
                };

                let item_content = Row {
                    gap: Some(12.0),
                    align_items: fission_ir::op::AlignItems::Center,
                    children: vec![
                        Checkbox {
                            checked: is_selected,
                            on_toggle: Some(ActionEnvelope {
                                id: select_id,
                                payload: serde_json::to_vec(&ToggleEmailSelection(email.id)).unwrap(),
                            }),
                            ..Default::default()
                        }.into_node(),
                        Container::new(
                            VStack {
                                spacing: Some(3.0),
                                children: vec![
                                HStack {
                                    spacing: Some(8.0),
                                    children: vec![
                                        Text::new(email.sender.clone()).size(16.0).into_node(),
                                        fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                                        Text::new(email.last_message().sent_at.format("%b %d").to_string())
                                            .size(14.0)
                                            .color(tokens.colors.text_secondary)
                                            .into_node(),
                                        Button {
                                            variant: ButtonVariant::Ghost,
                                            child: Some(Box::new(Icon::svg(star_icon).size(18.0).into_node())),
                                            on_press: Some(ActionEnvelope {
                                                id: flag_id,
                                                payload: serde_json::to_vec(&ToggleFlag(email.id)).unwrap(),
                                            }),
                                            width: Some(28.0),
                                            height: Some(28.0),
                                            padding: Some([4.0, 4.0, 0.0, 0.0]),
                                            ..Default::default()
                                        }.into_node(),
                                    ],
                                }.build(ctx, view),
                                Hero {
                                    tag: format!("email_subject_{}", email.id),
                                    child: Box::new(Text {
                                        content: TextContent::Literal(email.subject.clone()),
                                        font_size: Some(15.0),
                                        color: Some(subject_color),
                                        ..Default::default()
                                    }.into()),
                                }.build(ctx, view),
                                Text {
                                    content: TextContent::Literal(email.preview.chars().take(80).collect::<String>()),
                                    font_size: Some(13.0),
                                    color: Some(tokens.colors.text_secondary),
                                    max_height: Some(20.0),
                                    ..Default::default()
                                }.into(),
                                Wrap {
                                    direction: fission_ir::op::FlexDirection::Row,
                                    spacing: Some(6.0),
                                    children: email.labels.iter().map(|label| {
                                        Tag { label: label.clone(), on_close: None }.build(ctx, view)
                                    }).collect(),
                                }.build(ctx, view),
                                ]
                            }.build(ctx, view)
                        )
                        .flex_grow(1.0)
                        .into_node()
                    ],
                    ..Default::default()
                }.into_node();

                let item = VStack {
                    spacing: Some(0.0),
                    children: vec![
                        Container::new(item_content)
                            .padding_all(6.0)
                            .bg(if is_selected { tokens.colors.primary.with_alpha(20) } else { tokens.colors.surface })
                            .flex_grow(1.0)
                            .into_node(),
                        if idx + 1 < end_idx - start_idx {
                            Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view)
                        } else {
                            fission_core::ui::widgets::Spacer::default().into_node()
                        },
                    ],
                }.build(ctx, view);

                email_nodes.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        content_align: ButtonContentAlign::Start,
                        child: Some(Box::new(item)),
                        on_press: Some(ActionEnvelope {
                            id: navigate_id,
                            payload: serde_json::to_vec(&Navigate(path)).unwrap(),
                        }),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into()
                );
            }

            let lazy_id = WidgetNodeId::explicit("email_list");
            let node_id = NodeId::derived(lazy_id.as_u128(), &[current_page as u32]);

            list_items.push(
                LazyColumn {
                    id: Some(node_id),
                    children: Arc::new(email_nodes),
                    item_height: 0.0,
                }.into()
            );
        }
        
        // Pagination
        if !view.state.show_compose {
            list_items.push(
                fission_core::ui::widgets::Spacer { height: Some(20.0), ..Default::default() }.into_node()
            );
            list_items.push(
                fission_widgets::center::Center {
                    child: Box::new(Pagination {
                        current_page,
                        total_pages,
                        on_change: Some(Arc::new(move |page| {
                            ActionEnvelope {
                                id: page_id,
                                payload: serde_json::to_vec(&SetPage(page)).unwrap(),
                            }
                        })),
                    }.build(ctx, view))
                }.build(ctx, view)
            );
        }

        Container::new(
            VStack {
                spacing: Some(4.0),
                children: list_items,
            }
            .build(ctx, view)
        )
        .padding_all(8.0)
        .flex_grow(1.0)
        .bg(tokens.colors.background)
        .into_node()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use fission_test::TestHarness;
    use std::collections::HashSet;

    fn count_subject_text_nodes(h: &TestHarness<InboxState>) -> usize {
        let state = h.runtime.get_app_state::<InboxState>().unwrap();
        let subjects: HashSet<String> = state.emails.iter().map(|e| e.subject.clone()).collect();
        let ir = h.last_ir.as_ref().unwrap();
        ir.nodes
            .values()
            .filter(|n| {
                matches!(
                    &n.op,
                    fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. })
                        if subjects.contains(text)
                )
            })
            .count()
    }

    #[test]
    fn filter_mode_changes_list_contents() -> Result<()> {
        struct Root;
        impl Widget<InboxState> for Root {
            fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
                EmailList { folder: "inbox".into() }.build(ctx, view)
            }
        }

        let mut h = TestHarness::new(InboxState::default()).with_root_widget(Root);
        h.pump()?;

        let all_count = count_subject_text_nodes(&h);
        assert!(all_count > 0, "expected some subjects in All mode");

        h.dispatch(SetFilterMode(1))?; // Unread
        h.pump()?;
        let unread_count = count_subject_text_nodes(&h);
        assert!(unread_count < all_count, "Unread should show fewer items than All");

        h.dispatch(SetFilterMode(2))?; // Starred
        h.pump()?;
        let starred_count = count_subject_text_nodes(&h);
        assert!(starred_count < all_count, "Starred should show fewer items than All");

        Ok(())
    }
}
