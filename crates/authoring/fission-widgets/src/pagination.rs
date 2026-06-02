use crate::stack::HStack;
use fission_core::ui::{Button, ButtonVariant, Text, Widget};
use fission_core::ActionEnvelope;
use std::sync::Arc;

pub struct Pagination {
    pub current_page: usize,
    pub total_pages: usize,
    // Callback factory
    pub on_change: Option<Arc<dyn Fn(usize) -> ActionEnvelope + Send + Sync>>,
}

// We can't derive Serialize/Deserialize for closure.
// Manual impl or skip?
// Most widgets derive it for debug/inspector.
// We can skip this field.

impl std::fmt::Debug for Pagination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pagination")
            .field("current_page", &self.current_page)
            .field("total_pages", &self.total_pages)
            .finish()
    }
}

impl From<Pagination> for Widget {
    fn from(component: Pagination) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.pagination;
        let tokens = &view.env().theme.tokens;
        let mut children = Vec::new();

        let callback = |page: usize| this.on_change.as_ref().map(|f| f(page));

        // ... (Prev button) ...
        children.push(
            Button {
                variant: ButtonVariant::Outline,
                child: Some(Text::new("<").into()),
                on_press: if this.current_page > 1 {
                    callback(this.current_page - 1)
                } else {
                    None
                },
                disabled: this.current_page <= 1,
                width: Some(40.0),
                height: Some(40.0),
                padding: Some([0.0; 4]),
                ..Default::default()
            }
            .into(),
        );

        // ... (Pages logic) ...
        let start = (this.current_page as isize - 2).max(1) as usize;
        let end = (start + 4).min(this.total_pages);
        let start = (end as isize - 4).max(1) as usize;

        if start > 1 {
            children.push(page_btn(
                1,
                this.current_page == 1,
                callback(1),
                theme,
                tokens,
            ));
            if start > 2 {
                children.push(
                    Text::new("...")
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }
        }

        for i in start..=end {
            children.push(page_btn(
                i,
                this.current_page == i,
                callback(i),
                theme,
                tokens,
            ));
        }

        if end < this.total_pages {
            if end < this.total_pages - 1 {
                children.push(
                    Text::new("...")
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }
            children.push(page_btn(
                this.total_pages,
                this.current_page == this.total_pages,
                callback(this.total_pages),
                theme,
                tokens,
            ));
        }

        // ... (Next button) ...
        children.push(
            Button {
                variant: ButtonVariant::Outline,
                child: Some(Text::new(">").into()),
                on_press: if this.current_page < this.total_pages {
                    callback(this.current_page + 1)
                } else {
                    None
                },
                disabled: this.current_page >= this.total_pages,
                width: Some(40.0),
                height: Some(40.0),
                padding: Some([0.0; 4]),
                ..Default::default()
            }
            .into(),
        );

        HStack {
            spacing: Some(theme.spacing.max(6.0)),
            children,
        }
        .into()
    }
}

fn page_btn(
    page: usize,
    active: bool,
    action: Option<ActionEnvelope>,
    theme: &fission_theme::PaginationTheme,
    tokens: &fission_theme::Tokens,
) -> Widget {
    Button {
        variant: if active {
            ButtonVariant::Filled
        } else {
            ButtonVariant::Outline
        },
        child: Some(
            Text::new(format!("{}", page))
                .size(14.0)
                .color(if active {
                    theme.active_text
                } else {
                    tokens.colors.text_primary
                })
                .into(),
        ),
        on_press: action,
        width: Some(40.0),
        height: Some(40.0),
        padding: Some([0.0; 4]),
        ..Default::default()
    }
    .into()
}
