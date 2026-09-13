use crate::layout::{FIND_BAR_HEIGHT, TOOLBAR_CONTROL_SIZE};
use crate::model::*;
use crate::palette::EditorPalette;
use fission::core::ui::{Button, ButtonVariant, Container, Icon, Row, Text, TextInput, Widget};
use fission::core::{reduce_with, ActionEnvelope};
use fission::icons::material;
use fission::widgets::Spacer;

pub(crate) struct FindReplaceBar;

impl From<FindReplaceBar> for Widget {
    fn from(_component: FindReplaceBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        if !state.show_find_replace {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let total = state.find_matches.len();
        let match_label = if state.find_query.is_empty() || total == 0 {
            "No results".to_string()
        } else {
            format!("{} of {}", state.find_match_index + 1, total)
        };

        let inputs = Container::new(Row {
            children: vec![
                Container::new(TextInput {
                    id: Some(fission::WidgetId::explicit("find_input")),
                    value: state.find_query.clone(),
                    placeholder: Some("Find".into()),
                    on_input: Some(ctx.bind(UpdateFindQuery, reduce_with!(on_update_find_query))),
                    ..Default::default()
                })
                .flex_grow(1.0)
                .into(),
                Container::new(TextInput {
                    id: Some(fission::WidgetId::explicit("replace_input")),
                    value: state.replace_query.clone(),
                    placeholder: Some("Replace".into()),
                    on_input: Some(
                        ctx.bind(UpdateReplaceQuery, reduce_with!(on_update_replace_query)),
                    ),
                    ..Default::default()
                })
                .flex_grow(1.0)
                .into(),
            ],
            align_items: fission::op::AlignItems::Center,
            flex_grow: 1.0,
            ..Default::default()
        })
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
        .flex_grow(1.0)
        .into();

        Container::new(Row {
            children: vec![
                inputs,
                Container::new(
                    Text::new(match_label)
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text),
                )
                .padding_all(tokens.spacing.xs)
                .into(),
                FindBarIconButton {
                    icon: material::navigation::chevron_left::round(),
                    action: ctx.bind(FindPrevious, reduce_with!(on_find_previous)),
                }
                .into(),
                FindBarIconButton {
                    icon: material::navigation::chevron_right::round(),
                    action: ctx.bind(FindNext, reduce_with!(on_find_next)),
                }
                .into(),
                FindBarTextButton {
                    label: "Replace",
                    action: ctx.bind(ReplaceOne, reduce_with!(on_replace_one)),
                }
                .into(),
                FindBarTextButton {
                    label: "Replace All",
                    action: ctx.bind(ReplaceAll, reduce_with!(on_replace_all)),
                }
                .into(),
                FindBarIconButton {
                    icon: material::navigation::close::round(),
                    action: ctx.bind(ToggleFindReplace, reduce_with!(on_close_find_replace)),
                }
                .into(),
            ],
            align_items: fission::op::AlignItems::Center,
            ..Default::default()
        })
        .height(FIND_BAR_HEIGHT)
        .bg(palette.find_bar_bg)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
        .into()
    }
}

struct FindBarIconButton {
    icon: &'static str,
    action: ActionEnvelope,
}

impl From<FindBarIconButton> for Widget {
    fn from(button: FindBarIconButton) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Icon::svg(button.icon)
                    .size(tokens.typography.font_size_lg)
                    .color(palette.bright_text)
                    .into(),
            ),
            on_press: Some(button.action),
            height: Some(TOOLBAR_CONTROL_SIZE),
            width: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .into()
    }
}

struct FindBarTextButton {
    label: &'static str,
    action: ActionEnvelope,
}

impl From<FindBarTextButton> for Widget {
    fn from(button: FindBarTextButton) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new(button.label)
                    .size(tokens.typography.font_size_xs)
                    .color(palette.bright_text)
                    .into(),
            ),
            on_press: Some(button.action),
            height: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([
                tokens.spacing.none,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.s,
            ]),
            ..Default::default()
        }
        .into()
    }
}
