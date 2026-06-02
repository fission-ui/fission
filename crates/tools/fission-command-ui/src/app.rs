use super::components::{AppShell, ConfirmationDialog};
use super::screens::ActiveScreen;
use super::state::UiState;
use fission::prelude::*;

#[derive(Clone)]
pub struct CliUiApp;

impl From<CliUiApp> for Widget {
    fn from(_component: CliUiApp) -> Self {
        let (_ctx, view) = fission::build::current::<UiState>();
        let content = ActiveScreen.into();
        let shell = AppShell { content }.into();
        if view.state().pending_dialog.is_none() {
            return shell;
        }
        Overlay {
            content: shell,
            overlay: ConfirmationDialog.into(),
            ..Default::default()
        }
        .into()
    }
}
