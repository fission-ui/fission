use super::components::{AppShell, ConfirmationDialog};
use super::screens::ActiveScreen;
use super::state::UiState;
use fission::prelude::*;

#[derive(Clone)]
pub struct CliUiApp;

impl Widget<UiState> for CliUiApp {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let content = ActiveScreen.build(ctx, view);
        let shell = AppShell { content }.build(ctx, view);
        if view.state.pending_dialog.is_none() {
            return shell;
        }
        Overlay {
            content: Box::new(shell),
            overlay: Box::new(ConfirmationDialog.build(ctx, view)),
            ..Default::default()
        }
        .into_node()
    }
}
