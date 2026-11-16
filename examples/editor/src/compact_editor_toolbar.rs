use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) CompactEditorToolbar<EditorState> |_toolbar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let navigation_label = if state.sidebar_visible {
            "Editor"
        } else {
            "Files"
        };
        let navigation_icon = if state.sidebar_visible {
            fission::icons::material::action::description::round()
        } else {
            fission::icons::material::file::folder_open::regular()
        };

    } =>
        Container(
            Row(
            gap: tokens.spacing.s,
            align_items: ir_op::AlignItems::Center,
            children: [
                Button(
                    variant: ButtonVariant::Ghost,
                    child: Row(
                            gap: tokens.spacing.xs,
                            align_items: ir_op::AlignItems::Center,
                            children: [
                                Icon::svg(navigation_icon)
                                    .size(tokens.typography.font_size_lg)
                                    .color(palette.bright_text),
                                Text(navigation_label)
                                    .size(tokens.typography.font_size_sm)
                                    .color(palette.bright_text),
                            ],
                    ),
                    on_press: |_toolbar, state, _cx| {
                        state.sidebar_visible = !state.sidebar_visible;
                    },
                ).semantics_identifier("editor.compact.toggle-navigation"),
                Spacer(flex_grow: 1.0),
                Button(
                    variant: ButtonVariant::Ghost,
                    child: Some(Icon::svg(fission::icons::material::action::terminal::round())
                            .size(tokens.typography.font_size_lg)
                            .color(palette.bright_text).into()),
                    on_press: |_toolbar, state, _cx| {
                        state.terminal_visible = !state.terminal_visible;
                        if state.terminal_visible {
                            state.ensure_terminal_session();
                        }
                    },
                ).semantics_identifier("editor.compact.toggle-terminal"),
            ],
            ),
        )
        .bg(palette.surface_bg)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
}
