use crate::layout::STATUS_BAR_HEIGHT;
use crate::model::{DiagSeverity, EditorState};
use crate::palette::EditorPalette;
use fission::icons::material;
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

/// The branch, problem counts and cursor details on the left, and the latest
/// status message on the right.
pub struct StatusBar;

impl From<StatusBar> for Widget {
    fn from(_component: StatusBar) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let count = |severity: DiagSeverity| {
            state
                .diagnostics
                .values()
                .flatten()
                .filter(|diagnostic| diagnostic.severity == severity)
                .count()
        };
        let errors = count(DiagSeverity::Error);
        let warnings = count(DiagSeverity::Warning);

        let mut details: Vec<Widget> = widgets![
            StatusIconText {
                icon: material::notification::account_tree::round(),
                text: "main".into(),
                icon_color: palette.status_text,
            },
            HStack {
                spacing: Some(tokens.spacing.s),
                children: widgets![
                    StatusIconText {
                        icon: material::alert::error::round(),
                        text: errors.to_string(),
                        icon_color: if errors > 0 {
                            palette.status_error
                        } else {
                            palette.status_text
                        },
                    },
                    StatusIconText {
                        icon: material::alert::warning::round(),
                        text: warnings.to_string(),
                        icon_color: if warnings > 0 {
                            palette.status_warning
                        } else {
                            palette.status_text
                        },
                    },
                ],
            },
        ];
        if let Some((_tab, buf)) = state.active_buffer() {
            let cursor = format!("Ln {}, Col {}", buf.cursor_line + 1, buf.cursor_col + 1);
            details.extend(
                [
                    cursor,
                    buf.language.display_name().to_string(),
                    "UTF-8".to_string(),
                    buf.mode_label().to_string(),
                    "Spaces: 4".to_string(),
                ]
                .into_iter()
                .map(|text| StatusText { text }.into()),
            );
        }

        let mut children = widgets![
            HStack {
                spacing: Some(tokens.spacing.l),
                children: details,
            },
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            },
        ];
        if let Some(message) = &state.status_message {
            children.push(
                StatusText {
                    text: message.clone(),
                }
                .into(),
            );
        }

        Container::new(HStack {
            spacing: Some(tokens.spacing.none),
            children,
        })
        .bg(palette.surface_bg)
        .height(STATUS_BAR_HEIGHT)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
        .into()
    }
}

struct StatusText {
    text: String,
}

impl From<StatusText> for Widget {
    fn from(status: StatusText) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        Text::new(status.text)
            .size(view.env().theme.tokens.typography.font_size_sm)
            .color(palette.status_text)
            .into()
    }
}

struct StatusIconText {
    icon: &'static str,
    text: String,
    icon_color: Color,
}

impl From<StatusIconText> for Widget {
    fn from(status: StatusIconText) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        HStack {
            spacing: Some(tokens.spacing.xs),
            children: widgets![
                Icon::svg(status.icon)
                    .size(view.env().theme.components.button.icon_size)
                    .color(status.icon_color),
                StatusText { text: status.text },
            ],
        }
        .into()
    }
}
