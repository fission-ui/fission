use crate::editor_menu_item::EditorMenuItem;
use crate::model::{on_run_command, EditorCommand, EditorState, RunCommand};
use crate::palette::EditorPalette;
use fission::core::reduce_with;
use fission::core::ui::{Column, Container, Widget};

/// A dropdown card listing commands; used by the menu bar and context menus.
pub(crate) struct CommandMenu {
    pub commands: &'static [EditorCommand],
    pub width: f32,
}

impl From<CommandMenu> for Widget {
    fn from(menu: CommandMenu) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let items = menu
            .commands
            .iter()
            .map(|command| {
                EditorMenuItem::new(
                    command.label(),
                    ctx.bind(RunCommand(*command), reduce_with!(on_run_command)),
                )
                .into()
            })
            .collect();

        Container::new(Column {
            children: items,
            gap: Some(tokens.spacing.none),
            ..Default::default()
        })
        .width(menu.width)
        .bg(palette.flyout_bg)
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
        .into()
    }
}
