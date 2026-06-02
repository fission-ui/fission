use super::title_block;
use crate::components::KeyValueRow;
use crate::state::UiState;
use crate::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub struct HelpScreen;

impl From<HelpScreen> for Widget {
    fn from(_component: HelpScreen) -> Self {
        let (_ctx, view) = fission::build::current::<UiState>();
        let palette = UiPalette::for_mode(view.state().theme_mode);
        Column {
            gap: Some(1.0),
            children: vec![
                title_block(
                    "Help",
                    "Use the terminal UI when you want the CLI workflow without remembering each command and flag.",
                    palette.accent,
                    palette.muted,
                ),
                KeyValueRow::new("Quit", "press q or Esc").into(),
                KeyValueRow::new("Theme", "use Switch theme in the header").into(),
                KeyValueRow::new("Project", "initialise a directory and add targets from Project setup")
                    .into(),
                KeyValueRow::new("Run", "select a target, optionally select a device, then run")
                    .into(),
                KeyValueRow::new("Site", "build, check, serve, and inspect static routes")
                    .into(),
                KeyValueRow::new("Logs", "read a snapshot or start a follower under .fission/ui")
                    .into(),
                Text::new(
                    "The UI delegates actions to the same commands as the CLI, so command output and validation remain consistent.",
                )
                .color(palette.muted)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
