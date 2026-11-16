use crate::editor_menu_item::EditorMenuItem;
use crate::model::{on_run_command, EditorCommand, EditorState, RunCommand};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
/// A dropdown card listing commands; used by the menu bar and context menus.
    pub(crate) CommandMenu<EditorState> {
        commands: &'static [EditorCommand],
        width: f32,
    } |menu, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.none,
                children: [
                    for command in menu.commands {
                        EditorMenuItem(
                            command.label().into(),
                            cx.bind(RunCommand(*command), reduce_with!(on_run_command)),
                            key: command.label(),
                        )
                    },
                ],
            ),
        )
        .width(menu.width)
        .bg(palette.flyout_bg)
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
}
