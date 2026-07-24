use crate::model::{EditorState, ToggleSidebar, ToggleTerminal};
use crate::palette::{BRIGHT_TEXT, SURFACE_BG};
use fission::core::ui::{Button, ButtonVariant, Container, Row, Text, Widget};
use fission::core::with_reducer;
use fission::widgets::{Icon, Spacer};

pub(crate) struct CompactEditorToolbar;

impl From<CompactEditorToolbar> for Widget {
    fn from(_toolbar: CompactEditorToolbar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let toggle_sidebar = with_reducer!(
            ctx,
            ToggleSidebar,
            (|state: &mut EditorState, _: ToggleSidebar, _| {
                state.sidebar_visible = !state.sidebar_visible
            })
        );
        let toggle_terminal = with_reducer!(
            ctx,
            ToggleTerminal,
            (|state: &mut EditorState, _: ToggleTerminal, _| {
                state.terminal_visible = !state.terminal_visible;
                if state.terminal_visible {
                    state.ensure_terminal_session();
                }
            })
        );
        let navigation_label = if view.state().sidebar_visible {
            "Editor"
        } else {
            "Files"
        };
        let navigation_icon = if view.state().sidebar_visible {
            fission::icons::material::action::description::round()
        } else {
            fission::icons::material::file::folder_open::regular()
        };

        Container::new(Row {
            gap: Some(tokens.spacing.s),
            align_items: fission::op::AlignItems::Center,
            children: vec![
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Row {
                            gap: Some(tokens.spacing.xs),
                            align_items: fission::op::AlignItems::Center,
                            children: vec![
                                Icon::svg(navigation_icon)
                                    .size(tokens.typography.font_size_lg)
                                    .color(BRIGHT_TEXT)
                                    .into(),
                                Text::new(navigation_label)
                                    .size(tokens.typography.font_size_sm)
                                    .color(BRIGHT_TEXT)
                                    .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                    ),
                    on_press: Some(toggle_sidebar),
                    ..Default::default()
                }
                .semantics_identifier("editor.compact.toggle-navigation")
                .into(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Icon::svg(fission::icons::material::action::terminal::round())
                            .size(tokens.typography.font_size_lg)
                            .color(BRIGHT_TEXT)
                            .into(),
                    ),
                    on_press: Some(toggle_terminal),
                    ..Default::default()
                }
                .semantics_identifier("editor.compact.toggle-terminal")
                .into(),
            ],
            ..Default::default()
        })
        .bg(SURFACE_BG)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
        .into()
    }
}
