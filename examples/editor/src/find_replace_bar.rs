use crate::model::{EditorState, ToggleFindReplace, UpdateFindQuery, UpdateReplaceQuery, FindNext, FindPrevious, ReplaceOne, ReplaceAll};
use fission::core::op::Color;
use fission::core::ui::{Button, ButtonVariant, Container, Widget, Text, TextInput};
use fission::core::{BuildCtxHandle, reduce_with, ViewHandle};
use fission::widgets::{HStack, Spacer};

pub struct FindReplaceBar;

impl From<FindReplaceBar> for Widget {
    fn from(component: FindReplaceBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        if !view.state().show_find_replace {
            return Spacer { height: Some(0.0), ..Default::default() }.into();
        }

        let bg = Color { r: 37, g: 37, b: 38, a: 255 };
        let border = Color { r: 60, g: 60, b: 60, a: 255 };
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let dim = Color { r: 140, g: 140, b: 140, a: 255 };

        let update_find = ctx.bind(
            UpdateFindQuery(String::new()),
            reduce_with!((|s: &mut EditorState, a: UpdateFindQuery, _| {
                s.find_query = a.0;
                s.find_next(); // Auto-search as you type
            })),
        );

        let update_replace = ctx.bind(
            UpdateReplaceQuery(String::new()),
            reduce_with!((|s: &mut EditorState, a: UpdateReplaceQuery, _| s.replace_query = a.0)),
        );

        let find_next = ctx.bind(
            FindNext,
            reduce_with!((|s: &mut EditorState, _, _| s.find_next())),
        );

        let find_prev = ctx.bind(
            FindPrevious,
            reduce_with!((|s: &mut EditorState, _, _| s.find_previous())),
        );

        let replace_one = ctx.bind(
            ReplaceOne,
            reduce_with!((|s: &mut EditorState, _, _| s.replace_one())),
        );

        let replace_all = ctx.bind(
            ReplaceAll,
            reduce_with!((|s: &mut EditorState, _, _| s.replace_all())),
        );

        let close = ctx.bind(
            ToggleFindReplace,
            reduce_with!((|s: &mut EditorState, _, _| {
                s.show_find_replace = false;
                s.find_matches.clear();
            })),
        );

        // Match count
        let match_info = if view.state().find_matches.is_empty() {
            if view.state().find_query.is_empty() {
                String::new()
            } else {
                "No results".to_string()
            }
        } else {
            format!("{} of {}", view.state().find_match_index + 1, view.state().find_matches.len())
        };

        let small_btn = |label: &str, action: fission::core::ActionEnvelope| -> Widget {
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(
                    Text::new(label).size(11.0).color(text_color).into(),
                ),
                on_press: Some(action),
                height: Some(22.0),
                padding: Some([2.0, 4.0, 0.0, 0.0]),
                ..Default::default()
            }.into()
        };

        // Find row
        let find_row = HStack {
            spacing: Some(4.0),
            children: vec![
                Container::new(
                    TextInput {
                        id: Some(fission::WidgetId::explicit("editor_find_query_input")),
                        value: view.state().find_query.clone(),
                        placeholder: Some("Find".into()),
                        on_change: Some(update_find),
                        borderless: true,
                        ..Default::default()
                    },
                )
                .bg(Color { r: 60, g: 60, b: 60, a: 255 })
                .border(border, 1.0)
                .border_radius(2.0)
                .height(24.0)
                .flex_grow(1.0)
                .into(),
                Text::new(match_info).size(11.0).color(dim).into(),
                small_btn("^", find_prev),
                small_btn("v", find_next),
            ],
        }.into();

        // Replace row
        let replace_row = HStack {
            spacing: Some(4.0),
            children: vec![
                Container::new(
                    TextInput {
                        id: Some(fission::WidgetId::explicit("editor_replace_query_input")),
                        value: view.state().replace_query.clone(),
                        placeholder: Some("Replace".into()),
                        on_change: Some(update_replace),
                        borderless: true,
                        ..Default::default()
                    },
                )
                .bg(Color { r: 60, g: 60, b: 60, a: 255 })
                .border(border, 1.0)
                .border_radius(2.0)
                .height(24.0)
                .flex_grow(1.0)
                .into(),
                small_btn("Replace", replace_one),
                small_btn("All", replace_all),
            ],
        }.into();

        Container::new(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Container::new(
                        fission::widgets::VStack {
                            spacing: Some(4.0),
                            children: vec![find_row, replace_row],
                        },
                    ).flex_grow(1.0).into(),
                    small_btn("x", close),
                ],
            },
        )
        .bg(bg)
        .height(60.0)
        .padding_all(6.0)
        .border(Color { r: 48, g: 48, b: 49, a: 255 }, 1.0)
        .flex_shrink(0.0)
        .into()

    }
}
