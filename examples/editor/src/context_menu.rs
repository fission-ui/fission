use crate::command_menu::CommandMenu;
use crate::flyout_overlay::FlyoutOverlay;
use crate::layout::{
    CONTEXT_MENU_MAX_HEIGHT, CONTEXT_MENU_MAX_WIDTH, CONTEXT_MENU_MIN_WIDTH, OVERLAY_EDGE_GUTTER,
    OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::{
    on_dismiss_context_menu, DismissContextMenu, EditorState, EDITOR_CONTEXT_COMMANDS,
    TREE_CONTEXT_COMMANDS,
};
use crate::palette::EditorPalette;
use fission::core::ui::Widget;
use fission::core::{reduce_with, PortalLayer, WidgetId};
use fission::widgets::Spacer;

/// The right-click menu for the file tree and the editor, shown in a portal
/// at the pointer and kept inside the window.
pub(crate) struct ContextMenu;

impl From<ContextMenu> for Widget {
    fn from(_component: ContextMenu) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let state = view.state();
        if state.context_menu_visible {
            let palette = EditorPalette::from_theme(&view.env().theme);
            let viewport = view.viewport_size();
            let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
                .clamp(CONTEXT_MENU_MIN_WIDTH, CONTEXT_MENU_MAX_WIDTH);
            let (x, y) = state.context_menu_position;
            let commands = if state.context_menu_target.is_some() {
                TREE_CONTEXT_COMMANDS
            } else {
                EDITOR_CONTEXT_COMMANDS
            };
            let overlay = FlyoutOverlay {
                on_dismiss: ctx.bind(DismissContextMenu, reduce_with!(on_dismiss_context_menu)),
                backdrop: palette.interaction_backdrop,
                left: x.min((viewport.width - width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET)),
                top: y.min((viewport.height - CONTEXT_MENU_MAX_HEIGHT).max(OVERLAY_MIN_INSET)),
                child: CommandMenu { commands, width }.into(),
            };
            ctx.register_portal_with_layer(
                PortalLayer::Flyout,
                Some(WidgetId::explicit("context_menu")),
                overlay.into(),
            );
        }

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}
