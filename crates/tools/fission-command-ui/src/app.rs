use super::components::{AppShell, ConfirmationDialog};
use super::screens::ActiveScreen;
use super::state::UiState;
use fission::prelude::*;

#[derive(Clone)]
pub struct CliUiApp;

impl Widget<UiState> for CliUiApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<UiState>,
        view: &View<UiState>,
    ) -> impl fission::IntoWidget<UiState> {
        fission::AnyWidget::from_node({
            let content = ActiveScreen.build_node(ctx, view);
            let shell = AppShell { content }.build_node(ctx, view);
            if view.state.pending_dialog.is_none() {
                return fission::AnyWidget::from_node(shell);
            }
            Overlay {
                content: Box::new(shell),
                overlay: Box::new(ConfirmationDialog.build_node(ctx, view)),
                ..Default::default()
            }
            .into_node()
        })
    }
}
