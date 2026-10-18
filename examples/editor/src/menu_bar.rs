use crate::command_menu::CommandMenu;
use crate::editor_menu_button::EditorMenuButton;
use crate::flyout_overlay::FlyoutOverlay;
use crate::layout::{
    MENU_BAR_HEIGHT, MENU_BUTTON_WIDTH, MENU_FLYOUT_MAX_WIDTH, MENU_FLYOUT_MIN_WIDTH,
    OVERLAY_EDGE_GUTTER, OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::{
    on_dismiss_menu, on_toggle_menu, DismissMenu, EditorState, SetActiveMenu, MENUS,
};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) MenuBar<EditorState> |_bar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let active = state.active_menu.as_deref();
        if let Some(index) = MENUS.iter().position(|(label, _)| active == Some(*label)) {
            cx.register_portal_with_layer(
                PortalLayer::Modal,
                Some(WidgetId::explicit("menu_bar_flyout")),
                MenuFlyout::new(index).into(),
            );
        }
    } =>
        Container(
            Row(
                align_items: ir_op::AlignItems::Center,
                children: [
                    for (label, _) in MENUS {
                        EditorMenuButton(
                            label,
                            cx.bind(
                                SetActiveMenu(Some(label.to_string())),
                                reduce_with!(on_toggle_menu),
                            ),
                            key: label,
                        )
                    },
                    Spacer(flex_grow: 1.0),
                ],
            ),
        )
        .height(MENU_BAR_HEIGHT)
        .bg(palette.menu_bar_bg)
        .flex_shrink(0.0)
}

fission_component! {
/// The open menu's commands, under its button and kept inside the window.
    MenuFlyout<EditorState> { index: usize } |flyout, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let viewport = cx.view.viewport_size();
        let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(MENU_FLYOUT_MIN_WIDTH, MENU_FLYOUT_MAX_WIDTH);
        let left = (flyout.index as f32 * MENU_BUTTON_WIDTH)
            .min((viewport.width - width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));
        let (_, commands) = MENUS[flyout.index];
    } =>
        FlyoutOverlay(
            cx.bind(DismissMenu, reduce_with!(on_dismiss_menu)),
            palette.interaction_backdrop,
            left,
            MENU_BAR_HEIGHT,
            CommandMenu(commands, width),
        )
}
