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
use fission::prelude::*;

fission_component! {
/// The right-click menu for the file tree and the editor, shown in a portal
/// at the pointer and kept inside the window.
    pub(crate) ContextMenu<EditorState> |_menu, state, cx| {
        if state.context_menu_visible {
            let viewport = cx.view.viewport_size();
            let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
                .clamp(CONTEXT_MENU_MIN_WIDTH, CONTEXT_MENU_MAX_WIDTH);
            let (x, y) = state.context_menu_position;
            let commands = if state.context_menu_target.is_some() {
                TREE_CONTEXT_COMMANDS
            } else {
                EDITOR_CONTEXT_COMMANDS
            };
            cx.register_portal_with_layer(
                PortalLayer::Flyout,
                Some(WidgetId::explicit("context_menu")),
                ContextMenuOverlay::new(
                    commands,
                    width,
                    x.min((viewport.width - width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET)),
                    y.min((viewport.height - CONTEXT_MENU_MAX_HEIGHT).max(OVERLAY_MIN_INSET)),
                ).into(),
            );
        }
    } =>
    Spacer(height: 0.0)
}

fission_component! {
    ContextMenuOverlay<EditorState> {
        commands: &'static [crate::model::EditorCommand],
        width: f32,
        left: f32,
        top: f32,
    } |overlay, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
    } =>
    FlyoutOverlay(
        cx.bind(DismissContextMenu, reduce_with!(on_dismiss_context_menu)),
        palette.interaction_backdrop,
        overlay.left,
        overlay.top,
        CommandMenu(overlay.commands, overlay.width),
    )
}
