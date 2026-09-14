//! The list of threads in a folder, with its search, filters and pagination.

mod filters;
mod header;
mod row;
mod tabs;
mod toolbar;

use crate::model::list::set_page;
use crate::model::{Category, Email, Folder, InboxState, SetPage};
use fission::core::ui::{Button, Container, Text, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::icons::material;
use fission::widgets::center::Center;
use fission::widgets::{EmptyState, Icon, LazyColumn, Pagination, VStack};
use header::ListHeader;
use row::EmailRow;
use std::sync::Arc;
use tabs::FolderTabs;
use toolbar::ListToolbar;

pub struct EmailList {
    pub folder: String,
}

/// Layout decisions that depend on the space available.
///
/// Page size follows the viewport height as well as app state, so these stay
/// viewport reads rather than `Responsive` branches, gathered in one place.
#[derive(Clone, Copy)]
struct ListLayout {
    /// Narrow enough that navigation collapses behind a menu button.
    compact: bool,
    /// Rows drop their labels to fit more threads on screen.
    compact_rows: bool,
    /// Short enough that a page shows fewer threads.
    short: bool,
}

impl ListLayout {
    fn for_viewport(width: f32, height: f32) -> Self {
        Self {
            compact: width < 760.0,
            compact_rows: height < 680.0 || width < 980.0,
            short: height < 640.0,
        }
    }

    fn page_size(self) -> usize {
        if self.short {
            3
        } else if self.compact_rows {
            4
        } else {
            5
        }
    }
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

fn folder_path(folder: &Folder) -> String {
    match folder {
        Folder::Inbox => "inbox".into(),
        Folder::Starred => "starred".into(),
        Folder::Sent => "sent".into(),
        Folder::Drafts => "drafts".into(),
        Folder::Trash => "trash".into(),
        Folder::Custom(label) => label.to_lowercase(),
    }
}

impl From<EmailList> for Widget {
    fn from(component: EmailList) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let viewport = view.viewport_size();
        let layout = ListLayout::for_viewport(viewport.width, viewport.height);
        let folder = folder_from_route(&component.folder);

        let mut emails: Vec<&Email> = state
            .emails
            .iter()
            .filter(|email| email.folders.contains(&folder))
            .filter(|email| email.category == Category::from_tab(state.active_tab))
            .collect();
        if let Some(start) = state.date_filter.0 {
            emails.retain(|email| email.last_message().sent_at.date() >= start);
        }
        if let Some(end) = state.date_filter.1 {
            emails.retain(|email| email.last_message().sent_at.date() <= end);
        }
        let (min_mb, max_mb) = state.size_filter_mb;
        emails.retain(|email| {
            let size_mb = email.size_kb as f32 / 1000.0;
            size_mb >= min_mb && size_mb <= max_mb
        });
        if let Some(label) = &state.label_filter {
            emails.retain(|email| email.labels.contains(label));
        }
        if !state.search_query.trim().is_empty() {
            emails.retain(|email| email.matches_query(&state.search_query));
        }
        match state.filter_mode {
            1 => emails.retain(|email| !email.is_read),
            2 => emails.retain(|email| email.is_flagged),
            _ => {}
        }
        match state.sort_option.as_str() {
            "Oldest" => emails.sort_by_key(|email| email.last_message().sent_at),
            "Unread" => emails.sort_by_key(|email| email.is_read),
            _ => emails.sort_by_key(|email| std::cmp::Reverse(email.last_message().sent_at)),
        }

        let page_size = layout.page_size();
        let total_pages = emails.len().div_ceil(page_size).max(1);
        let current_page = state.page.clamp(1, total_pages);
        let start = (current_page - 1) * page_size;
        let end = (start + page_size).min(emails.len());
        let unread_count = state
            .emails
            .iter()
            .filter(|email| email.folders.contains(&folder) && !email.is_read)
            .count();

        let list_body: Widget = if emails.is_empty() {
            EmptyState {
                icon: Some(
                    Icon::svg(material::content::inbox::regular())
                        .size(tokens.spacing.xxl)
                        .color(tokens.colors.text_primary)
                        .into(),
                ),
                title: view.tr("empty.no_emails"),
                description: Some(view.tr("empty.caught_up")),
                action: Some(
                    Button {
                        child: Some(Text::new(view.tr("action.refresh")).into()),
                        on_press: None,
                        ..Default::default()
                    }
                    .into(),
                ),
            }
            .into()
        } else {
            let page = &emails[start..end];
            let path = folder_path(&folder);
            Container::new(LazyColumn {
                id: Some(WidgetId::explicit(&format!(
                    "email_list_page_{current_page}"
                ))),
                children: page
                    .iter()
                    .enumerate()
                    .map(|(index, email)| {
                        EmailRow {
                            email: (*email).clone(),
                            path: format!("/{path}/{}", email.id),
                            compact: layout.compact_rows,
                            last: index + 1 == page.len(),
                        }
                        .into()
                    })
                    .collect(),
                item_height: 0.0,
            })
            .flex_grow(1.0)
            .min_height(0.0)
            .into()
        };

        let mut children: Vec<Widget> = vec![
            VStack {
                spacing: Some(tokens.spacing.xs),
                children: vec![
                    ListHeader {
                        folder: folder.clone(),
                        unread_count,
                        compact: layout.compact,
                    }
                    .into(),
                    ListToolbar.into(),
                    FolderTabs.into(),
                ],
            }
            .into(),
            Container::new(list_body)
                .flex_grow(1.0)
                .min_height(0.0)
                .into(),
        ];
        if !state.show_compose {
            let pages: Vec<_> = (0..=total_pages)
                .map(|page| ctx.bind(SetPage(page), reduce_with!(set_page)))
                .collect();
            children.push(
                Center {
                    child: Pagination {
                        current_page,
                        total_pages,
                        on_change: Some(Arc::new(move |page| pages[page.min(total_pages)].clone())),
                    }
                    .into(),
                }
                .into(),
            );
        }

        Container::new(VStack {
            spacing: Some(tokens.spacing.s),
            children,
        })
        .padding_all(tokens.spacing.s)
        .flex_grow(1.0)
        .bg(tokens.colors.background)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SetFilterMode;
    use anyhow::Result;
    use fission_test::TestHarness;
    use std::collections::HashSet;

    fn visible_subject_texts(h: &TestHarness<InboxState>) -> HashSet<String> {
        let state = h.runtime.get_app_state::<InboxState>().unwrap();
        let subjects: HashSet<String> = state.emails.iter().map(|e| e.subject.clone()).collect();
        let ir = h.last_ir.as_ref().unwrap();
        ir.nodes
            .values()
            .filter_map(|n| n.op.text())
            .filter(|text| subjects.contains(text.as_ref()))
            .map(|text| text.into_owned())
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
