use fission_core::ui::{Button, ButtonVariant, Node, Text};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use fission_core::op::Color;
use crate::stack::HStack;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

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

impl<S: fission_core::AppState> Widget<S> for Pagination {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();
        
        let callback = |page: usize| {
            self.on_change.as_ref().map(|f| f(page))
        };

        // Prev
        children.push(
            Button {
                variant: ButtonVariant::Outline,
                child: Some(Box::new(Text::new("<").into_node())),
                on_press: if self.current_page > 1 { callback(self.current_page - 1) } else { None },
                disabled: self.current_page <= 1,
                width: Some(32.0), height: Some(32.0),
                ..Default::default()
            }.into_node()
        );

        // Simple logic: Show all if small, or sliding window.
        // For MVP: Show max 5 pages centered.
        // Plus First/Last?
        // Logic: 1 ... k-1 k k+1 ... N
        
        let start = (self.current_page as isize - 2).max(1) as usize;
        let end = (start + 4).min(self.total_pages);
        let start = (end as isize - 4).max(1) as usize; // Re-clamp start if near end

        if start > 1 {
             children.push(page_btn(1, self.current_page == 1, callback(1), tokens));
             if start > 2 {
                 children.push(Text::new("...").into_node());
             }
        }

        for i in start..=end {
            children.push(page_btn(i, self.current_page == i, callback(i), tokens));
        }

        if end < self.total_pages {
             if end < self.total_pages - 1 {
                 children.push(Text::new("...").into_node());
             }
             children.push(page_btn(self.total_pages, self.current_page == self.total_pages, callback(self.total_pages), tokens));
        }

        // Next
        children.push(
            Button {
                variant: ButtonVariant::Outline,
                child: Some(Box::new(Text::new(">").into_node())),
                on_press: if self.current_page < self.total_pages { callback(self.current_page + 1) } else { None },
                disabled: self.current_page >= self.total_pages,
                width: Some(32.0), height: Some(32.0),
                ..Default::default()
            }.into_node()
        );

        HStack {
            spacing: Some(8.0),
            children,
        }.into_node()
    }
}

fn page_btn(page: usize, active: bool, action: Option<ActionEnvelope>, tokens: &fission_theme::Tokens) -> Node {
    Button {
        variant: if active { ButtonVariant::Filled } else { ButtonVariant::Ghost },
        child: Some(Box::new(
            Text::new(format!("{}", page))
                .color(if active { tokens.colors.on_primary } else { tokens.colors.text_primary })
                .into_node()
        )),
        on_press: action,
        width: Some(32.0), height: Some(32.0),
        ..Default::default()
    }.into_node()
}
