use crate::model::{
    Folder, InboxState, Navigate, SelectTab, SetAdvancedFiltersOpen, SetFilterMode, SetPage,
    SetSortOption, ToggleEmailSelection, ToggleFilterDropdown, ToggleFlag, UpdateSearch,
};
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Checkbox, Container, Row, Text, TextContent, Widget,
};
use fission::core::ActionEnvelope;
use fission::core::{reduce_with, WidgetId};
use fission::icons::material;
use fission::theme::ComponentSize;
use fission::widgets::{
    Badge, DateRangePicker, Divider, DropDown, EmptyState, HStack, Hero, Icon, LazyColumn,
    Pagination, Popover, RangeSlider, SegmentedControl, TabItem, Tabs, Tag, TextInput, VStack,
    Wrap,
};
use serde_json;
use std::sync::Arc;

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

impl From<EmailList> for Widget {
    fn from(component: EmailList) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let t = |key: &str| {
            view.env()
                .i18n
                .get(&view.env().locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let mut chrome_items = vec![];

        let folder = folder_from_route(&component.folder);
        let compact_rows =
            view.viewport_size().height < 680.0 || view.viewport_size().width < 980.0;
        let short_viewport = view.viewport_size().height < 640.0;
        let filters_width = (view.viewport_size().width * 0.34).clamp(240.0, 320.0);
        let folder_label = match &folder {
            Folder::Inbox => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.inbox")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Inbox".into()),
            Folder::Starred => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.starred")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Starred".into()),
            Folder::Sent => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.sent")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Sent".into()),
            Folder::Drafts => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.drafts")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Drafts".into()),
            Folder::Trash => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.trash")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Trash".into()),
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
            .state()
            .emails
            .iter()
            .filter(|e| e.folders.contains(&folder) && !e.is_read)
            .count();

        // Register handlers and get IDs
        let filter_id = ctx
            .bind(
                SetFilterMode(0),
                reduce_with!((|s: &mut InboxState, a: SetFilterMode, _| s.filter_mode = a.0)),
            )
            .id;
        let page_id = ctx
            .bind(
                SetPage(0),
                reduce_with!((|s: &mut InboxState, a: SetPage, _| s.page = a.0)),
            )
            .id;
        let filters_open_id = ctx
            .bind(
                SetAdvancedFiltersOpen(false),
                reduce_with!(
                    (|s: &mut InboxState, a: SetAdvancedFiltersOpen, _| s.show_advanced_filters =
                        a.0)
                ),
            )
            .id;
        let sort_id = ctx
            .bind(
                SetSortOption("Newest".into()),
                reduce_with!((|s: &mut InboxState, a: SetSortOption, _| s.sort_option = a.0)),
            )
            .id;
        let search_id = ctx
            .bind(
                UpdateSearch("".into()),
                reduce_with!((|s: &mut InboxState, a: UpdateSearch, _| s.search_query = a.0)),
            )
            .id;
        let select_id = ctx
            .bind(
                ToggleEmailSelection(0),
                reduce_with!(
                    (|s: &mut InboxState, a: ToggleEmailSelection, _| {
                        if let Some(pos) = s.selected_emails.iter().position(|id| *id == a.0) {
                            s.selected_emails.remove(pos);
                        } else {
                            s.selected_emails.push(a.0);
                        }
                    })
                ),
            )
            .id;
        let flag_id = ctx
            .bind(
                ToggleFlag(0),
                reduce_with!(
                    (|s: &mut InboxState, a: ToggleFlag, _| {
                        if let Some(email) = s.emails.iter_mut().find(|e| e.id == a.0) {
                            email.is_flagged = !email.is_flagged;
                            if email.is_flagged {
                                email.folders.insert(Folder::Starred);
                            } else {
                                email.folders.remove(&Folder::Starred);
                            }
                        }
                    })
                ),
            )
            .id;
        let navigate_id = ctx
            .bind(
                Navigate("".into()),
                reduce_with!(
                    (|s: &mut InboxState, a: Navigate, _| {
                        s.navigate_to(a.0);
                        s.show_mobile_menu = false;
                        if let Some(id) = s.selected_email_id {
                            if let Some(email) = s.emails.iter_mut().find(|e| e.id == id) {
                                email.is_read = true;
                            }
                        }
                    })
                ),
            )
            .id;
        let tab_id = ctx
            .bind(
                SelectTab(0),
                reduce_with!((|s: &mut InboxState, a: SelectTab, _| s.active_tab = a.0)),
            )
            .id;
        let _menu_toggle = ctx.bind(
            ToggleFilterDropdown,
            reduce_with!(
                (|s: &mut InboxState, _: ToggleFilterDropdown, _| {
                    s.show_filter_dropdown = !s.show_filter_dropdown
                })
            ),
        );

        // Header
        chrome_items.push(
            Row {
                gap: Some(6.0),
                children: vec![
                    Text::new(folder_label).size(20.0).into(),
                    Badge {
                        text: format!("{} {}", unread_count, t("badge.new")),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        );

        // Filter + Search row
        let sort_toggle = if view.state().sort_option == "Newest" {
            "Oldest"
        } else {
            "Newest"
        };
        let sort_toggle = ActionEnvelope {
            id: sort_id,
            payload: serde_json::to_vec(&SetSortOption(sort_toggle.into())).unwrap(),
        };
        // Search row
        chrome_items.push(
            TextInput {
                value: view.state().search_query.clone(),
                placeholder: Some(TextContent::Key("search.placeholder".into())),
                on_change: Some(ActionEnvelope {
                    id: search_id,
                    payload: Vec::new(),
                }),
                ..Default::default()
            }
            .into(),
        );

        // Filter row
        chrome_items.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    SegmentedControl {
                        options: vec!["All".into(), "Unread".into(), "Starred".into()],
                        selected_index: view.state().filter_mode,
                        on_change: Some(Arc::new(move |idx| ActionEnvelope {
                            id: filter_id,
                            payload: serde_json::to_vec(&SetFilterMode(idx)).unwrap(),
                        })),
                    }
                    .into(),
                    fission::core::ui::widgets::Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into(),
                    DropDown {
                        selected: Some(view.state().sort_option.clone()),
                        options: vec!["Newest".into(), "Oldest".into(), "Unread".into()],
                        on_toggle: Some(sort_toggle),
                        ..Default::default()
                    }
                    .into(),
                    Popover {
                        id: WidgetId::explicit("advanced_filters"),
                        is_open: view.state().show_advanced_filters,
                        on_toggle: Some(ActionEnvelope {
                            id: filters_open_id,
                            payload: serde_json::to_vec(&SetAdvancedFiltersOpen(
                                !view.state().show_advanced_filters,
                            ))
                            .unwrap(),
                        }),
                        on_close: Some(ActionEnvelope {
                            id: filters_open_id,
                            payload: serde_json::to_vec(&SetAdvancedFiltersOpen(false)).unwrap(),
                        }),
                        trigger: Button {
                            variant: ButtonVariant::Outline,
                            child: Some(
                                HStack {
                                    spacing: Some(6.0),
                                    children: vec![
                                        Icon::svg(material::content::filter_list::regular())
                                            .size(18.0)
                                            .into(),
                                        Text::new(TextContent::Key("header.filters".into())).into(),
                                    ],
                                }
                                .into(),
                            ),
                            on_press: Some(ActionEnvelope {
                                id: filters_open_id,
                                payload: serde_json::to_vec(&SetAdvancedFiltersOpen(
                                    !view.state().show_advanced_filters,
                                ))
                                .unwrap(),
                            }),
                            ..Default::default()
                        }
                        .into(),
                        content: Container::new(VStack {
                            spacing: Some(14.0),
                            children: vec![
                                Text::new(TextContent::Key("filter.date_range".into()))
                                    .size(12.0)
                                    .color(tokens.colors.text_secondary)
                                    .into(),
                                DateRangePicker {
                                    id_start: WidgetId::explicit("filter_date_start"),
                                    id_end: WidgetId::explicit("filter_date_end"),
                                    start: view.state().schedule_date,
                                    end: view.state().schedule_date,
                                    is_start_open: false,
                                    is_end_open: false,
                                    on_change: None,
                                    on_toggle_start: None,
                                    on_toggle_end: None,
                                    on_close_start: None,
                                    on_close_end: None,
                                }
                                .into(),
                                Text::new(TextContent::Key("filter.size_mb".into()))
                                    .size(12.0)
                                    .color(tokens.colors.text_secondary)
                                    .into(),
                                RangeSlider {
                                    id: None,
                                    start: 5.0,
                                    end: 50.0,
                                    min: 0.0,
                                    max: 100.0,
                                    on_change: None,
                                }
                                .into(),
                            ],
                        })
                        .width(filters_width)
                        .padding_all(14.0)
                        .bg(tokens.colors.surface)
                        .border(tokens.colors.border, 1.0)
                        .border_radius(tokens.radii.medium)
                        .shadow(
                            tokens
                                .elevations
                                .level2
                                .unwrap_or(fission::core::op::BoxShadow {
                                    color: fission::core::op::Color {
                                        r: 0,
                                        g: 0,
                                        b: 0,
                                        a: 40,
                                    },
                                    blur_radius: 10.0,
                                    offset: (0.0, 4.0),
                                }),
                        )
                        .into(),
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        );

        chrome_items.push(
            Tabs {
                active_index: view.state().active_tab,
                items: vec![
                    TabItem {
                        title: t("tabs.primary"),
                        content: fission::core::ui::widgets::Spacer::default().into(),
                        on_press: Some(ActionEnvelope {
                            id: tab_id,
                            payload: serde_json::to_vec(&SelectTab(0)).unwrap(),
                        }),
                    },
                    TabItem {
                        title: t("tabs.social"),
                        content: fission::core::ui::widgets::Spacer::default().into(),
                        on_press: Some(ActionEnvelope {
                            id: tab_id,
                            payload: serde_json::to_vec(&SelectTab(1)).unwrap(),
                        }),
                    },
                    TabItem {
                        title: t("tabs.promotions"),
                        content: fission::core::ui::widgets::Spacer::default().into(),
                        on_press: Some(ActionEnvelope {
                            id: tab_id,
                            payload: serde_json::to_vec(&SelectTab(2)).unwrap(),
                        }),
                    },
                ],
                size: ComponentSize::Sm,
            }
            .into(),
        );

        let mut emails: Vec<_> = view
            .state()
            .emails
            .iter()
            .filter(|e| e.folders.contains(&folder))
            .collect();

        if !view.state().search_query.trim().is_empty() {
            emails.retain(|e| e.matches_query(&view.state().search_query));
        }

        match view.state().filter_mode {
            0 => {}
            1 => emails.retain(|e| !e.is_read),
            2 => emails.retain(|e| e.is_flagged),
            _ => {}
        }

        match view.state().sort_option.as_str() {
            "Oldest" => emails.sort_by_key(|e| e.last_message().sent_at),
            "Unread" => emails.sort_by_key(|e| e.is_read),
            _ => emails.sort_by_key(|e| std::cmp::Reverse(e.last_message().sent_at)),
        }

        let page_size = if short_viewport {
            3
        } else if compact_rows {
            4
        } else {
            5
        };
        let total_pages = ((emails.len() + page_size - 1) / page_size).max(1);
        let current_page = view.state().page.max(1).min(total_pages);
        let start_idx = (current_page - 1) * page_size;
        let end_idx = (start_idx + page_size).min(emails.len());

        let list_body: Widget = if emails.is_empty() {
            EmptyState {
                icon: Some(
                    Icon::svg(material::content::inbox::regular())
                        .size(48.0)
                        .color(tokens.colors.text_primary)
                        .into(),
                ),
                title: t("empty.no_emails"),
                description: Some(t("empty.caught_up")),
                action: Some(
                    Button {
                        child: Some(Text::new(t("action.refresh")).into()),
                        on_press: None,
                        ..Default::default()
                    }
                    .into(),
                ),
            }
            .into()
        } else {
            let mut email_nodes = Vec::new();
            for (idx, email) in emails[start_idx..end_idx].iter().enumerate() {
                let path = format!("/{}/{}", folder_path, email.id);
                let is_selected = view.state().selected_emails.contains(&email.id);
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

                let item_content: Widget = Row {
                    gap: Some(12.0),
                    align_items: fission::op::AlignItems::Center,
                    children: vec![
                        Checkbox {
                            checked: is_selected,
                            on_toggle: Some(ActionEnvelope {
                                id: select_id,
                                payload: serde_json::to_vec(&ToggleEmailSelection(email.id))
                                    .unwrap(),
                            }),
                            ..Default::default()
                        }
                        .into(),
                        Container::new(VStack {
                            spacing: Some(4.0),
                            children: vec![
                                HStack {
                                    spacing: Some(8.0),
                                    children: vec![
                                        Text::new(email.sender.clone()).size(16.0).into(),
                                        fission::core::ui::widgets::Spacer {
                                            flex_grow: 1.0,
                                            ..Default::default()
                                        }
                                        .into(),
                                        Text::new(
                                            email
                                                .last_message()
                                                .sent_at
                                                .format("%b %d")
                                                .to_string(),
                                        )
                                        .size(14.0)
                                        .color(tokens.colors.text_secondary)
                                        .into(),
                                        Button {
                                            variant: ButtonVariant::Ghost,
                                            child: Some(Icon::svg(star_icon).size(18.0).into()),
                                            on_press: Some(ActionEnvelope {
                                                id: flag_id,
                                                payload: serde_json::to_vec(&ToggleFlag(email.id))
                                                    .unwrap(),
                                            }),
                                            width: Some(28.0),
                                            height: Some(28.0),
                                            padding: Some([4.0, 4.0, 0.0, 0.0]),
                                            ..Default::default()
                                        }
                                        .into(),
                                    ],
                                }
                                .into(),
                                Hero {
                                    tag: format!("email_subject_{}", email.id),
                                    child: Text {
                                        content: TextContent::Literal(email.subject.clone()),
                                        font_size: Some(15.0),
                                        color: Some(subject_color),
                                        ..Default::default()
                                    }
                                    .into(),
                                }
                                .into(),
                                Container::new(Text {
                                    content: TextContent::Literal({
                                        let preview: String =
                                            email.preview.chars().take(45).collect();
                                        if email.preview.chars().count() > 45 {
                                            format!("{}...", preview)
                                        } else {
                                            preview
                                        }
                                    }),
                                    font_size: Some(13.0),
                                    color: Some(tokens.colors.text_secondary),
                                    max_height: Some(16.0),
                                    ..Default::default()
                                })
                                .flex_grow(1.0)
                                .flex_shrink(1.0)
                                .into(),
                                if compact_rows || email.labels.is_empty() {
                                    fission::core::ui::widgets::Spacer::default().into()
                                } else {
                                    Wrap {
                                        direction: fission::op::FlexDirection::Row,
                                        spacing: Some(6.0),
                                        children: email
                                            .labels
                                            .iter()
                                            .map(|label| {
                                                Tag {
                                                    label: label.clone(),
                                                    on_close: None,
                                                }
                                                .into()
                                            })
                                            .collect(),
                                    }
                                    .into()
                                },
                            ],
                        })
                        .flex_grow(1.0)
                        .into(),
                    ],
                    ..Default::default()
                }
                .into();

                let item: Widget = VStack {
                    spacing: Some(0.0),
                    children: vec![
                        Container::new(item_content)
                            .padding_all(4.0)
                            .bg(if is_selected {
                                tokens.colors.primary.with_alpha(20)
                            } else {
                                tokens.colors.surface
                            })
                            .flex_grow(1.0)
                            .into(),
                        if idx + 1 < end_idx - start_idx {
                            Divider {
                                orientation: fission::widgets::divider::Orientation::Horizontal,
                            }
                            .into()
                        } else {
                            fission::core::ui::widgets::Spacer::default().into()
                        },
                    ],
                }
                .into();

                email_nodes.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        content_align: ButtonContentAlign::Start,
                        child: Some(Container::new(item).flex_grow(1.0).min_width(0.0).into()),
                        on_press: Some(ActionEnvelope {
                            id: navigate_id,
                            payload: serde_json::to_vec(&Navigate(path)).unwrap(),
                        }),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into(),
                );
            }

            let node_id = WidgetId::explicit(&format!("email_list_page_{current_page}"));

            Container::new(LazyColumn {
                id: Some(node_id),
                children: email_nodes,
                item_height: 0.0,
            })
            .flex_grow(1.0)
            .min_height(0.0)
            .into()
        };

        let footer = if !view.state().show_compose {
            VStack {
                spacing: Some(4.0),
                children: vec![
                    fission::core::ui::widgets::Spacer {
                        height: Some(4.0),
                        ..Default::default()
                    }
                    .into(),
                    fission::widgets::center::Center {
                        child: Pagination {
                            current_page,
                            total_pages,
                            on_change: Some(Arc::new(move |page| ActionEnvelope {
                                id: page_id,
                                payload: serde_json::to_vec(&SetPage(page)).unwrap(),
                            })),
                        }
                        .into(),
                    }
                    .into(),
                ],
            }
            .into()
        } else {
            fission::core::ui::widgets::Spacer::default().into()
        };

        Container::new(VStack {
            spacing: Some(8.0),
            children: vec![
                VStack {
                    spacing: Some(4.0),
                    children: chrome_items,
                }
                .into(),
                Container::new(list_body)
                    .flex_grow(1.0)
                    .min_height(0.0)
                    .into(),
                footer,
            ],
        })
        .padding_all(8.0)
        .flex_grow(1.0)
        .bg(tokens.colors.background)
        .into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use fission_test::TestHarness;
    use std::collections::HashSet;

    fn visible_subject_texts(h: &TestHarness<InboxState>) -> HashSet<String> {
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
            .filter_map(|n| match &n.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. })
                    if subjects.contains(text) =>
                {
                    Some(text.clone())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn filter_mode_changes_list_contents() -> Result<()> {
        #[derive(Clone)]
        struct Root;
        impl From<Root> for Widget {
            fn from(_component: Root) -> Self {
                let (_ctx, _view) = fission::build::current::<InboxState>();
                EmailList {
                    folder: "inbox".into(),
                }
                .into()
            }
        }
        let mut h = TestHarness::new(InboxState::default()).with_root_widget(Root);
        h.pump()?;

        let all_subjects = visible_subject_texts(&h);
        assert!(
            !all_subjects.is_empty(),
            "expected some subjects in All mode"
        );

        h.dispatch(SetFilterMode(1))?; // Unread
        h.pump()?;
        let unread_subjects = visible_subject_texts(&h);
        assert_ne!(
            unread_subjects, all_subjects,
            "Unread mode should change the visible subject set"
        );
        assert!(
            !unread_subjects.contains("Design review: Inbox refresh"),
            "read threads should disappear in Unread mode"
        );
        assert!(
            unread_subjects.contains("Quarterly planning sync"),
            "known unread threads should remain visible in Unread mode"
        );

        h.dispatch(SetFilterMode(2))?; // Starred
        h.pump()?;
        let starred_subjects = visible_subject_texts(&h);
        assert_ne!(
            starred_subjects, all_subjects,
            "Starred mode should change the visible subject set"
        );
        assert!(
            starred_subjects.contains("Design review: Inbox refresh"),
            "flagged threads should remain visible in Starred mode"
        );

        Ok(())
    }
}
