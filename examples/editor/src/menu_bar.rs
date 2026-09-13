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
use fission::core::ui::{Container, Row, Widget};
use fission::core::{reduce_with, PortalLayer, WidgetId};
use fission::widgets::Spacer;

pub(crate) struct MenuBar;

impl From<MenuBar> for Widget {
    fn from(_component: MenuBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let mut buttons: Vec<Widget> = MENUS
            .iter()
            .map(|(label, _)| {
                EditorMenuButton {
                    label,
                    action: ctx.bind(
                        SetActiveMenu(Some((*label).to_string())),
                        reduce_with!(on_toggle_menu),
                    ),
                }
                .into()
            })
            .collect();
        buttons.push(
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
        );

        let active = view.state().active_menu.as_deref();
        if let Some(index) = MENUS.iter().position(|(label, _)| active == Some(*label)) {
            ctx.register_portal_with_layer(
                PortalLayer::Modal,
                Some(WidgetId::explicit("menu_bar_flyout")),
                MenuFlyout { index }.into(),
            );
        }

        Container::new(Row {
            children: buttons,
            align_items: fission::op::AlignItems::Center,
            ..Default::default()
        })
        .height(MENU_BAR_HEIGHT)
        .bg(palette.menu_bar_bg)
        .flex_shrink(0.0)
        .into()
    }
}

/// The open menu's commands, under its button and kept inside the window.
struct MenuFlyout {
    index: usize,
}

impl From<MenuFlyout> for Widget {
    fn from(flyout: MenuFlyout) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let viewport = view.viewport_size();
        let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(MENU_FLYOUT_MIN_WIDTH, MENU_FLYOUT_MAX_WIDTH);
        let left = (flyout.index as f32 * MENU_BUTTON_WIDTH)
            .min((viewport.width - width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));
        let (_, commands) = MENUS[flyout.index];

        FlyoutOverlay {
            on_dismiss: ctx.bind(DismissMenu, reduce_with!(on_dismiss_menu)),
            backdrop: palette.interaction_backdrop,
            left,
            top: MENU_BAR_HEIGHT,
            child: CommandMenu { commands, width }.into(),
        }
        .into()
    }
}
