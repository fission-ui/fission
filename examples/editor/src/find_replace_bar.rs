use crate::layout::{FIND_BAR_HEIGHT, TOOLBAR_CONTROL_SIZE};
use crate::model::*;
use crate::palette::EditorPalette;
use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
enum FindCommand {
    Previous,
    Next,
    Replace,
    ReplaceAll,
    Close,
}

fission_component! {
    pub(crate) FindReplaceBar<EditorState> |_bar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let total = state.find_matches.len();
        let match_label = if state.find_query.is_empty() || total == 0 {
            "No results".to_string()
        } else {
            format!("{} of {}", state.find_match_index + 1, total)
        };
    } =>
        if state.show_find_replace {
            Container(
                Row(
                    align_items: ir_op::AlignItems::Center,
                    children: [
                        Container(
                            Row(
                                align_items: ir_op::AlignItems::Center,
                                flex_grow: 1.0,
                                children: [
                                    Container(
                                        TextInput(
                                            key: "find-query",
                                            id: WidgetId::explicit("find_input"),
                                            value: state.find_query.clone(),
                                            placeholder: Some("Find".into()),
                                            on_input: |_bar, state, cx| {
                                                if let Some(change) = cx.input.text_change() {
                                                    state.find_query = change.new_text.clone();
                                                    state.find_next();
                                                }
                                            },
                                        ),
                                    ).flex_grow(1.0),
                                    Container(
                                        TextInput(
                                            id: WidgetId::explicit("replace_input"),
                                            placeholder: Some("Replace".into()),
                                            bind: state.replace_query,
                                        ),
                                    ).flex_grow(1.0),
                                ],
                            ),
                        )
                        .border(palette.flyout_border, 1.0)
                        .border_radius(tokens.radii.small)
                        .flex_grow(1.0),
                        Container(
                            Text(match_label)
                                .size(tokens.typography.font_size_xs)
                                .color(palette.dim_text),
                        ).padding_all(tokens.spacing.xs),
                        FindBarIconButton(
                            fission::icons::material::navigation::chevron_left::round(),
                            FindCommand::Previous,
                            "Previous match",
                        ),
                        FindBarIconButton(
                            fission::icons::material::navigation::chevron_right::round(),
                            FindCommand::Next,
                            "Next match",
                        ),
                        FindBarTextButton(
                            "Replace",
                            FindCommand::Replace,
                        ),
                        FindBarTextButton(
                            "Replace All",
                            FindCommand::ReplaceAll,
                        ),
                        FindBarIconButton(
                            fission::icons::material::navigation::close::round(),
                            FindCommand::Close,
                            "Close find and replace",
                        ),
                    ],
                ),
            )
            .height(FIND_BAR_HEIGHT)
            .bg(palette.find_bar_bg)
            .padding_all(tokens.spacing.xs)
            .flex_shrink(0.0)
        } else {
            Spacer(height: 0.0)
        }
}

fission_component! {
    FindBarIconButton<EditorState> {
        icon: &'static str,
        command: FindCommand,
        label: &'static str,
    } |button, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            child: Some(Icon::svg(button.icon)
                .size(tokens.typography.font_size_lg)
                .color(palette.bright_text).into()),
            on_press: |button, state, _cx| run_find_command(state, button.command),
            height: TOOLBAR_CONTROL_SIZE,
            width: TOOLBAR_CONTROL_SIZE,
            padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
            semantics: Semantics {
                label: Some(button.label.into()),
                ..Default::default()
            },
        )
}

fission_component! {
    FindBarTextButton<EditorState> {
        label: &'static str,
        command: FindCommand,
    } |button, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            child: Text(button.label)
                .size(tokens.typography.font_size_xs)
                .color(palette.bright_text),
            on_press: |button, state, _cx| run_find_command(state, button.command),
            height: TOOLBAR_CONTROL_SIZE,
            padding: [
                tokens.spacing.none,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.s,
            ],
        )
}

fn run_find_command(state: &mut EditorState, command: FindCommand) {
    match command {
        FindCommand::Previous => state.find_previous(),
        FindCommand::Next => state.find_next(),
        FindCommand::Replace => state.replace_one(),
        FindCommand::ReplaceAll => state.replace_all(),
        FindCommand::Close => state.show_find_replace = false,
    }
}
