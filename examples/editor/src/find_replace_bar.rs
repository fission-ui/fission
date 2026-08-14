use crate::layout::{FIND_BAR_HEIGHT, TOOLBAR_CONTROL_SIZE};
use crate::model::*;
use crate::palette::{BRIGHT_TEXT, DIM_TEXT, FIND_BAR_BG, FLYOUT_BORDER};
use fission::core::ui::{Button, ButtonVariant, Container, Icon, Row, Text, TextInput, Widget};
use fission::core::{reduce_with, ReducerContext};
use fission::icons::material;
use fission::widgets::Spacer;

pub(crate) struct FindReplaceBar;

impl From<FindReplaceBar> for Widget {
    fn from(_component: FindReplaceBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        if !view.state().show_find_replace {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let update_find = ctx.bind(
            UpdateFindQuery,
            reduce_with!(
                (|s: &mut EditorState,
                  _a: UpdateFindQuery,
                  ctx: &mut ReducerContext<EditorState>| {
                    let Some(change) = ctx.input.text_change() else {
                        return;
                    };
                    s.find_query = change.new_text.clone();
                    s.find_next(); // Auto-search as you type
                })
            ),
        );

        let update_replace = ctx.bind(
            UpdateReplaceQuery,
            reduce_with!(
                (|s: &mut EditorState,
                  _a: UpdateReplaceQuery,
                  ctx: &mut ReducerContext<EditorState>| {
                    if let Some(change) = ctx.input.text_change() {
                        s.replace_query = change.new_text.clone();
                    }
                })
            ),
        );

        let close_find = ctx.bind(
            ToggleFindReplace,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_find_replace = false;
                })
            ),
        );

        let find_next = ctx.bind(
            FindNext,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.find_next();
                })
            ),
        );

        let find_prev = ctx.bind(
            FindPrevious,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.find_previous();
                })
            ),
        );

        let replace_one = ctx.bind(
            ReplaceOne,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.replace_one();
                })
            ),
        );

        let replace_all_action = ctx.bind(
            ReplaceAll,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.replace_all();
                })
            ),
        );

        // Match count display
        let total = view.state().find_matches.len();
        let current = if total > 0 {
            view.state().find_match_index + 1
        } else {
            0
        };
        let match_label = if view.state().find_query.is_empty() {
            "No results".to_string()
        } else if total == 0 {
            "No results".to_string()
        } else {
            format!("{} of {}", current, total)
        };

        let find_input = Container::new(TextInput {
            id: Some(fission::WidgetId::explicit("find_input")),
            value: view.state().find_query.clone(),
            placeholder: Some("Find".into()),
            on_input: Some(update_find),
            ..Default::default()
        })
        .flex_grow(1.0)
        .into();

        let replace_input = Container::new(TextInput {
            id: Some(fission::WidgetId::explicit("replace_input")),
            value: view.state().replace_query.clone(),
            placeholder: Some("Replace".into()),
            on_input: Some(update_replace),
            ..Default::default()
        })
        .flex_grow(1.0)
        .into();

        let match_text: Widget = Text::new(match_label.clone())
            .size(tokens.typography.font_size_xs)
            .color(DIM_TEXT)
            .into();

        let btn_prev = Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Icon::svg(material::navigation::chevron_left::round())
                    .size(tokens.typography.font_size_lg)
                    .color(BRIGHT_TEXT)
                    .into(),
            ),
            on_press: Some(find_prev),
            height: Some(TOOLBAR_CONTROL_SIZE),
            width: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .into();

        let btn_next = Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Icon::svg(material::navigation::chevron_right::round())
                    .size(tokens.typography.font_size_lg)
                    .color(BRIGHT_TEXT)
                    .into(),
            ),
            on_press: Some(find_next),
            height: Some(TOOLBAR_CONTROL_SIZE),
            width: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .into();

        let btn_replace = Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new("Replace")
                    .size(tokens.typography.font_size_xs)
                    .color(BRIGHT_TEXT)
                    .into(),
            ),
            on_press: Some(replace_one),
            height: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([
                tokens.spacing.none,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.s,
            ]),
            ..Default::default()
        }
        .into();

        let btn_replace_all = Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new("Replace All")
                    .size(tokens.typography.font_size_xs)
                    .color(BRIGHT_TEXT)
                    .into(),
            ),
            on_press: Some(replace_all_action),
            height: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([
                tokens.spacing.none,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.s,
            ]),
            ..Default::default()
        }
        .into();

        let btn_close = Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Icon::svg(material::navigation::close::round())
                    .size(tokens.typography.font_size_lg)
                    .color(BRIGHT_TEXT)
                    .into(),
            ),
            on_press: Some(close_find),
            height: Some(TOOLBAR_CONTROL_SIZE),
            width: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .into();

        Container::new(Row {
            children: vec![
                Container::new(Row {
                    children: vec![find_input, replace_input],
                    align_items: fission::op::AlignItems::Center,
                    flex_grow: 1.0,
                    ..Default::default()
                })
                .border(FLYOUT_BORDER, 1.0)
                .border_radius(tokens.radii.small)
                .flex_grow(1.0)
                .into(),
                Container::new(match_text)
                    .padding_all(tokens.spacing.xs)
                    .into(),
                btn_prev,
                btn_next,
                btn_replace,
                btn_replace_all,
                btn_close,
            ],
            align_items: fission::op::AlignItems::Center,
            ..Default::default()
        })
        .height(FIND_BAR_HEIGHT)
        .bg(FIND_BAR_BG)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
        .into()
    }
}
