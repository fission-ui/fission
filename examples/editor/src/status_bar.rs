use crate::layout::STATUS_BAR_HEIGHT;
use crate::model::{DiagSeverity, EditorState};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
/// The branch, problem counts and cursor details on the left, and the latest
/// status message on the right.
    pub StatusBar<EditorState> |_bar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
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
    } =>
        Container(
            HStack(
                spacing: tokens.spacing.none,
                children: [
                    HStack(
                        spacing: tokens.spacing.l,
                        children: [
                            StatusIconText(
                                fission::icons::material::notification::account_tree::round(),
                                "main".to_string(),
                                palette.status_text,
                            ),
                            HStack(
                                spacing: tokens.spacing.s,
                                children: [
                                    StatusIconText(
                                        fission::icons::material::alert::error::round(),
                                        errors.to_string(),
                                        if errors > 0 {
                                            palette.status_error
                                        } else {
                                            palette.status_text
                                        },
                                    ),
                                    StatusIconText(
                                        fission::icons::material::alert::warning::round(),
                                        warnings.to_string(),
                                        if warnings > 0 {
                                            palette.status_warning
                                        } else {
                                            palette.status_text
                                        },
                                    ),
                                ],
                            ),
                            if let Some((_tab, buffer)) = state.active_buffer() {
                                StatusText(format!(
                                    "Ln {}, Col {}",
                                    buffer.cursor_line + 1,
                                    buffer.cursor_col + 1,
                                ))
                            },
                            if let Some((_tab, buffer)) = state.active_buffer() {
                                StatusText(buffer.language.display_name().to_string())
                            },
                            if state.active_buffer().is_some() {
                                StatusText("UTF-8".to_string())
                            },
                            if let Some((_tab, buffer)) = state.active_buffer() {
                                StatusText(buffer.mode_label().to_string())
                            },
                            if state.active_buffer().is_some() {
                                StatusText("Spaces: 4".to_string())
                            },
                        ],
                    ),
                    Spacer(flex_grow: 1.0),
                    if let Some(message) = &state.status_message {
                        StatusText(message.clone())
                    },
                ],
            ),
        )
        .bg(palette.surface_bg)
        .height(STATUS_BAR_HEIGHT)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
}

fission_component! {
    StatusText<EditorState> { text: String } |status, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
    } =>
        Text(status.text)
            .size(cx.view.env().theme.tokens.typography.font_size_sm)
            .color(palette.status_text)
}

fission_component! {
    StatusIconText<EditorState> {
        icon: &'static str,
        text: String,
        icon_color: Color,
    } |status, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        HStack(
            spacing: tokens.spacing.xs,
            children: [
                Icon::svg(status.icon)
                    .size(cx.view.env().theme.components.button.icon_size)
                    .color(status.icon_color),
                StatusText(status.text.clone()),
            ],
        )
}
