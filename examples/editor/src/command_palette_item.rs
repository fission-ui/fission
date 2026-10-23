use crate::layout::PANEL_HEADER_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) CommandPaletteItem<EditorState> {
        label: &'static str,
        description: &'static str,
        action: ActionEnvelope,
    } |item, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: HStack(
                    spacing: tokens.spacing.m,
                    children: [
                        Text(item.label)
                            .size(tokens.typography.font_size_sm)
                            .color(palette.bright_text),
                        Spacer(flex_grow: 1.0),
                        Text(item.description)
                            .size(tokens.typography.font_size_xs)
                            .color(palette.dim_text),
                    ],
            ),
            on_press: item.action.clone(),
            height: PANEL_HEADER_HEIGHT,
            padding: [
                tokens.spacing.s,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.none,
            ],
        )
        .semantics_identifier(format!(
            "editor.command.{}",
            item.label.to_lowercase().replace(' ', "-")
        ))
}
