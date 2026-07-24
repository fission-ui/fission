use crate::layout::STATUS_BAR_HEIGHT;
use crate::model::EditorState;
use crate::palette::{STATUS_ERROR, STATUS_TEXT, STATUS_WARNING, SURFACE_BG};
use fission::icons::material;
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub struct StatusBar;

impl From<StatusBar> for Widget {
    fn from(_component: StatusBar) -> Self {
        let (_ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let icon_size = view.env().theme.components.button.icon_size;

        let mut items = vec![];

        items.push(
            HStack {
                spacing: Some(tokens.spacing.xs),
                children: widgets![
                    Icon::svg(material::notification::account_tree::round())
                        .size(icon_size)
                        .color(STATUS_TEXT),
                    Text::new("main")
                        .size(tokens.typography.font_size_sm)
                        .color(STATUS_TEXT),
                ],
            }
            .into(),
        );

        items.push(
            Spacer {
                width: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into(),
        );

        let error_count: usize = view
            .state()
            .diagnostics
            .values()
            .flat_map(|d| d.iter())
            .filter(|d| d.severity == crate::model::DiagSeverity::Error)
            .count();
        let warn_count: usize = view
            .state()
            .diagnostics
            .values()
            .flat_map(|d| d.iter())
            .filter(|d| d.severity == crate::model::DiagSeverity::Warning)
            .count();

        items.push(
            HStack {
                spacing: Some(tokens.spacing.xs),
                children: widgets![
                    Icon::svg(material::alert::error::round())
                        .size(icon_size)
                        .color(if error_count > 0 {
                            STATUS_ERROR
                        } else {
                            STATUS_TEXT
                        }),
                    Text::new(error_count.to_string())
                        .size(tokens.typography.font_size_sm)
                        .color(STATUS_TEXT),
                ],
            }
            .into(),
        );
        items.push(
            Spacer {
                width: Some(tokens.spacing.s),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            HStack {
                spacing: Some(tokens.spacing.xs),
                children: widgets![
                    Icon::svg(material::alert::warning::round())
                        .size(icon_size)
                        .color(if warn_count > 0 {
                            STATUS_WARNING
                        } else {
                            STATUS_TEXT
                        }),
                    Text::new(warn_count.to_string())
                        .size(tokens.typography.font_size_sm)
                        .color(STATUS_TEXT),
                ],
            }
            .into(),
        );

        items.push(
            Spacer {
                width: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into(),
        );

        if let Some((_tab, buf)) = view.state().active_buffer() {
            items.push(
                Text::new(format!(
                    "Ln {}, Col {}",
                    buf.cursor_line + 1,
                    buf.cursor_col + 1
                ))
                .size(tokens.typography.font_size_sm)
                .color(STATUS_TEXT)
                .into(),
            );

            items.push(
                Spacer {
                    width: Some(tokens.spacing.l),
                    ..Default::default()
                }
                .into(),
            );

            items.push(
                Text::new(buf.language.display_name())
                    .size(tokens.typography.font_size_sm)
                    .color(STATUS_TEXT)
                    .into(),
            );

            items.push(
                Spacer {
                    width: Some(tokens.spacing.l),
                    ..Default::default()
                }
                .into(),
            );

            items.push(
                Text::new("UTF-8")
                    .size(tokens.typography.font_size_sm)
                    .color(STATUS_TEXT)
                    .into(),
            );

            items.push(
                Spacer {
                    width: Some(tokens.spacing.l),
                    ..Default::default()
                }
                .into(),
            );

            items.push(
                Text::new(buf.mode_label())
                    .size(tokens.typography.font_size_sm)
                    .color(STATUS_TEXT)
                    .into(),
            );

            items.push(
                Spacer {
                    width: Some(tokens.spacing.l),
                    ..Default::default()
                }
                .into(),
            );

            items.push(
                Text::new("Spaces: 4")
                    .size(tokens.typography.font_size_sm)
                    .color(STATUS_TEXT)
                    .into(),
            );
        }

        items.push(
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
        );

        if let Some(msg) = &view.state().status_message {
            items.push(
                Text::new(msg.clone())
                    .size(tokens.typography.font_size_sm)
                    .color(STATUS_TEXT)
                    .into(),
            );
        }

        Container::new(HStack {
            spacing: Some(tokens.spacing.none),
            children: items,
        })
        .bg(SURFACE_BG)
        .height(STATUS_BAR_HEIGHT)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
        .into()
    }
}
