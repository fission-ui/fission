use super::title_block;
use crate::actions::{
    set_scrollback_limit, set_scrollback_limit_input, toggle_compact_mode, SetScrollbackLimit,
    SetScrollbackLimitInput, ToggleCompactMode,
};
use crate::components::{ActionButton, ButtonTone, FormTextField, KeyValueRow};
use crate::state::UiState;
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct SettingsScreen;

impl Widget<UiState> for SettingsScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let set_limit_input = with_reducer!(
            ctx,
            SetScrollbackLimitInput(String::new()),
            set_scrollback_limit_input
        );
        let compact = with_reducer!(ctx, ToggleCompactMode, toggle_compact_mode);
        let presets = [10_000usize, 100_000, 500_000, 1_000_000]
            .into_iter()
            .map(|limit| {
                let action = with_reducer!(ctx, SetScrollbackLimit(limit), set_scrollback_limit);
                ActionButton::new(format_scrollback_limit(limit), action)
                    .tone(if view.state.scrollback_limit == limit {
                        ButtonTone::Primary
                    } else {
                        ButtonTone::Neutral
                    })
                    .width(16.0)
                    .build(ctx, view)
            })
            .collect::<Vec<_>>();

        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Settings",
                    "Tune terminal UI behaviour without changing the project configuration.",
                    palette.accent,
                    palette.muted,
                ),
                KeyValueRow::new(
                    "Density",
                    if view.state.compact_mode {
                        "Compact".to_string()
                    } else {
                        "Comfortable".to_string()
                    },
                )
                .build(ctx, view),
                ActionButton::new(
                    if view.state.compact_mode {
                        "Use comfortable spacing"
                    } else {
                        "Use compact spacing"
                    },
                    compact,
                )
                .tone(ButtonTone::Neutral)
                .width(28.0)
                .build(ctx, view),
                KeyValueRow::new(
                    "Scrollback",
                    format!("{} lines", view.state.scrollback_limit),
                )
                .build(ctx, view),
                FormTextField::new(
                    "cli_ui_scrollback_limit",
                    "Scrollback lines",
                    view.state.scrollback_limit_input.clone(),
                    "100000, 500k, 1m",
                    set_limit_input,
                )
                .width(28.0)
                .build(ctx, view),
                Row {
                    gap: Some(1.0),
                    children: presets,
                    ..Default::default()
                }
                .into_node(),
                Text::new(
                    "The UI keeps a bounded ring buffer for command output. Older lines are discarded once the configured limit is reached, which keeps long-running sessions predictable instead of growing memory without limit.",
                )
                .color(palette.muted)
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}

fn format_scrollback_limit(limit: usize) -> String {
    if limit >= 1_000_000 && limit % 1_000_000 == 0 {
        format!("{}M", limit / 1_000_000)
    } else if limit >= 1_000 && limit % 1_000 == 0 {
        format!("{}K", limit / 1_000)
    } else {
        limit.to_string()
    }
}
