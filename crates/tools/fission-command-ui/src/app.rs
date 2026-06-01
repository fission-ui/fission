use super::components::{AppShell, ConfirmationDialog};
use super::screens::ActiveScreen;
use super::state::UiState;
use fission::prelude::*;
use fission::IntoWidget;

#[derive(Clone)]
pub struct CliUiApp;

impl Widget<UiState> for CliUiApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<UiState>,
        view: &View<UiState>,
    ) -> impl fission::IntoWidget<UiState> {
        fission::core::view::internal_node_widget({
            let content = ActiveScreen
                .build(ctx, view)
                .into_widget()
                .lower_to_node(ctx, view);
            let shell = AppShell { content }
                .build(ctx, view)
                .into_widget()
                .lower_to_node(ctx, view);
            if view.state.pending_dialog.is_none() {
                return fission::core::view::internal_node_widget(shell);
            }
            Overlay {
                content: Box::new(shell),
                overlay: Box::new(
                    ConfirmationDialog
                        .build(ctx, view)
                        .into_widget()
                        .lower_to_node(ctx, view),
                ),
                ..Default::default()
            }
            .into_node()
        })
    }
}
