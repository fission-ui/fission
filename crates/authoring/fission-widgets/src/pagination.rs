use crate::{stack::HStack, Icon};
use fission_core::op::{BoxAlignment, LineCap, LineJoin, Stroke};
use fission_core::ui::{
    Button, ButtonStyleOverride, ButtonVariant, Container, Text, TextContent, Widget,
};
use fission_core::{ActionEnvelope, LayoutDirection, Semantics};
use fission_icons::material;
use fission_theme::ResolvedComponentStyle;
use std::sync::Arc;

/// Controlled page-number navigation with previous and next controls.
pub struct Pagination {
    /// Current one-based page number.
    pub current_page: usize,
    /// Total number of available pages.
    pub total_pages: usize,
    /// Factory producing an action for a requested one-based page number.
    pub on_change: Option<Arc<dyn Fn(usize) -> ActionEnvelope + Send + Sync>>,
}

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
        let theme = &view.env().theme.components.pagination;
        let mut children = Vec::new();

        let callback = |page: usize| component.on_change.as_ref().map(|factory| factory(page));
        children.push(
            PaginationControl {
                kind: PaginationControlKind::Previous,
                active: false,
                action: (component.current_page > 1)
                    .then(|| callback(component.current_page - 1))
                    .flatten(),
                disabled: component.current_page <= 1,
            }
            .into(),
        );

        let start = (component.current_page as isize - 2).max(1) as usize;
        let end = (start + 4).min(component.total_pages);
        let start = (end as isize - 4).max(1) as usize;

        if start > 1 {
            children.push(
                PaginationControl {
                    kind: PaginationControlKind::Page(1),
                    active: component.current_page == 1,
                    action: callback(1),
                    disabled: false,
                }
                .into(),
            );
            if start > 2 {
                children.push(PaginationEllipsis.into());
            }
        }

        for page in start..=end {
            children.push(
                PaginationControl {
                    kind: PaginationControlKind::Page(page),
                    active: component.current_page == page,
                    action: callback(page),
                    disabled: false,
                }
                .into(),
            );
        }

        if end < component.total_pages {
            if end < component.total_pages - 1 {
                children.push(PaginationEllipsis.into());
            }
            children.push(
                PaginationControl {
                    kind: PaginationControlKind::Page(component.total_pages),
                    active: component.current_page == component.total_pages,
                    action: callback(component.total_pages),
                    disabled: false,
                }
                .into(),
            );
        }

        children.push(
            PaginationControl {
                kind: PaginationControlKind::Next,
                active: false,
                action: (component.current_page < component.total_pages)
                    .then(|| callback(component.current_page + 1))
                    .flatten(),
                disabled: component.current_page >= component.total_pages,
            }
            .into(),
        );

        HStack {
            spacing: Some(theme.spacing),
            children,
        }
        .into()
    }
}

#[derive(Clone, Copy, Debug)]
enum PaginationControlKind {
    Previous,
    Page(usize),
    Next,
}

#[derive(Clone, Debug)]
struct PaginationControl {
    kind: PaginationControlKind,
    active: bool,
    action: Option<ActionEnvelope>,
    disabled: bool,
}

impl From<PaginationControl> for Widget {
    fn from(component: PaginationControl) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.pagination;
        let tokens = &view.env().theme.tokens;
        let style = if component.active {
            let mut selected = theme.selected_style.clone();
            selected
                .background
                .get_or_insert(fission_core::op::Fill::Solid(theme.active_bg));
            selected.text_color.get_or_insert(theme.active_text);
            theme.item_style.merge(&selected)
        } else {
            theme.item_style.clone()
        };

        let (child, label) = match component.kind {
            PaginationControlKind::Previous => {
                let icon = if view.env().layout_direction == LayoutDirection::RightToLeft {
                    material::navigation::chevron_right::regular()
                } else {
                    material::navigation::chevron_left::regular()
                };
                (
                    Icon::svg(icon)
                        .size(style.icon_size.unwrap_or(tokens.spacing.m))
                        .color(style.text_color.unwrap_or(tokens.colors.text_primary))
                        .into(),
                    TextContent::KeyWithFallback {
                        key: "fission.pagination.previous".into(),
                        fallback: "Previous page".into(),
                    }
                    .resolve(view.env()),
                )
            }
            PaginationControlKind::Page(page) => {
                let page = page.to_string();
                (Text::new(page.clone()).into(), page)
            }
            PaginationControlKind::Next => {
                let icon = if view.env().layout_direction == LayoutDirection::RightToLeft {
                    material::navigation::chevron_left::regular()
                } else {
                    material::navigation::chevron_right::regular()
                };
                (
                    Icon::svg(icon)
                        .size(style.icon_size.unwrap_or(tokens.spacing.m))
                        .color(style.text_color.unwrap_or(tokens.colors.text_primary))
                        .into(),
                    TextContent::KeyWithFallback {
                        key: "fission.pagination.next".into(),
                        fallback: "Next page".into(),
                    }
                    .resolve(view.env()),
                )
            }
        };

        Button {
            variant: if component.active {
                ButtonVariant::Outline
            } else {
                ButtonVariant::TertiaryGray
            },
            child: Some(child),
            on_press: component.action,
            disabled: component.disabled,
            semantics: Some(Semantics {
                role: fission_ir::Role::Button,
                label: Some(label),
                selected: Some(component.active),
                ..Default::default()
            }),
            style: Some(button_override(&style)),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PaginationEllipsis;

impl From<PaginationEllipsis> for Widget {
    fn from(_: PaginationEllipsis) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.pagination.ellipsis_style;
        let tokens = &view.env().theme.tokens;
        let width = style.width.or(style.height).unwrap_or(tokens.spacing.l);
        let height = style.height.unwrap_or(width);

        Container::new(
            Text::new("…")
                .size(style.font_size.unwrap_or(tokens.typography.font_size_base))
                .weight(
                    style
                        .font_weight
                        .unwrap_or(tokens.typography.font_weight_regular),
                )
                .line_height(style.line_height.unwrap_or(height))
                .color(style.text_color.unwrap_or(tokens.colors.text_muted)),
        )
        .size(width, height)
        .align_child(BoxAlignment::Center)
        .into()
    }
}

fn button_override(style: &ResolvedComponentStyle) -> ButtonStyleOverride {
    ButtonStyleOverride {
        background_fill: style.background.clone(),
        text_color: style.text_color,
        border: style.border.as_ref().map(|border| Stroke {
            fill: border.fill.clone(),
            width: border.width,
            dash_array: None,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
        }),
        width: style.width,
        height: style.height,
        padding: Some(style.padding_box(0.0, 0.0)),
        corner_radius: style.radius,
        shadows: Some(style.outer_shadows()),
        font_size: style.font_size,
        font_weight: style.font_weight,
        line_height: style.line_height,
        letter_spacing: style.letter_spacing,
        ..Default::default()
    }
}
