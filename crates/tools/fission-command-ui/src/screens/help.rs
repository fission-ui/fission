use super::title_block;
use crate::components::KeyValueRow;
use crate::state::UiState;
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct HelpScreen;

impl Widget<UiState> for HelpScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Help",
                    "Use the terminal UI when you want the CLI workflow without remembering each command and flag.",
                    palette.accent,
                    palette.muted,
                ),
                KeyValueRow::new("Quit", "press q or Esc").build(ctx, view),
                KeyValueRow::new("Theme", "use Switch theme in the header").build(ctx, view),
                KeyValueRow::new("Project", "initialise a directory and add targets from Project setup")
                    .build(ctx, view),
                KeyValueRow::new("Run", "select a target, optionally select a device, then run")
                    .build(ctx, view),
                KeyValueRow::new("Site", "build, check, serve, and inspect static routes")
                    .build(ctx, view),
                KeyValueRow::new("Logs", "read a snapshot or start a follower under .fission/ui")
                    .build(ctx, view),
                Text::new(
                    "The UI delegates actions to the same commands as the CLI, so command output and validation remain consistent.",
                )
                .color(palette.muted)
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}
