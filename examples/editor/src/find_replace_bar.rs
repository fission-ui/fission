use crate::model::{EditorState, ToggleFindReplace, UpdateFindQuery, UpdateReplaceQuery, FindNext, FindPrevious, ReplaceOne, ReplaceAll};
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonVariant, Container, Node, Text, TextInput};
use fission_core::{BuildCtx, Handler, View, Widget};
use fission_widgets::{HStack, Spacer};

pub struct FindReplaceBar;

impl Widget<EditorState> for FindReplaceBar {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        if !view.state.show_find_replace {
            return Spacer { height: Some(0.0), ..Default::default() }.into_node();
        }

        let bg = Color { r: 37, g: 37, b: 38, a: 255 };
        let border = Color { r: 60, g: 60, b: 60, a: 255 };
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let dim = Color { r: 140, g: 140, b: 140, a: 255 };

        let update_find = ctx.bind(
            UpdateFindQuery(String::new()),
            (|s: &mut EditorState, a: UpdateFindQuery, _| {
                s.find_query = a.0;
                s.find_next(); // Auto-search as you type
            }) as Handler<EditorState, UpdateFindQuery>,
        );

        let update_replace = ctx.bind(
            UpdateReplaceQuery(String::new()),
            (|s: &mut EditorState, a: UpdateReplaceQuery, _| s.replace_query = a.0)
                as Handler<EditorState, UpdateReplaceQuery>,
        );

        let find_next = ctx.bind(
            FindNext,
            (|s: &mut EditorState, _, _| s.find_next())
                as Handler<EditorState, FindNext>,
        );

        let find_prev = ctx.bind(
            FindPrevious,
            (|s: &mut EditorState, _, _| s.find_previous())
                as Handler<EditorState, FindPrevious>,
        );

        let replace_one = ctx.bind(
            ReplaceOne,
            (|s: &mut EditorState, _, _| s.replace_one())
                as Handler<EditorState, ReplaceOne>,
        );

        let replace_all = ctx.bind(
            ReplaceAll,
            (|s: &mut EditorState, _, _| s.replace_all())
                as Handler<EditorState, ReplaceAll>,
        );

        let close = ctx.bind(
            ToggleFindReplace,
            (|s: &mut EditorState, _, _| {
                s.show_find_replace = false;
                s.find_matches.clear();
            }) as Handler<EditorState, ToggleFindReplace>,
        );

        // Match count
        let match_info = if view.state.find_matches.is_empty() {
            if view.state.find_query.is_empty() {
                String::new()
            } else {
                "No results".to_string()
            }
        } else {
            format!("{} of {}", view.state.find_match_index + 1, view.state.find_matches.len())
        };

        let small_btn = |label: &str, action: fission_core::ActionEnvelope| -> Node {
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Text::new(label).size(11.0).color(text_color).into_node(),
                )),
                on_press: Some(action),
                height: Some(22.0),
                padding: Some([2.0, 4.0, 0.0, 0.0]),
                ..Default::default()
            }.into_node()
        };

        // Find row
        let find_row = HStack {
            spacing: Some(4.0),
            children: vec![
                Container::new(
                    TextInput {
                        id: Some(fission_ir::NodeId::explicit("editor_find_query_input")),
                        value: view.state.find_query.clone(),
                        placeholder: Some("Find".into()),
                        on_change: Some(update_find),
                        borderless: true,
                        ..Default::default()
                    }.into_node(),
                )
                .bg(Color { r: 60, g: 60, b: 60, a: 255 })
                .border(border, 1.0)
                .border_radius(2.0)
                .height(24.0)
                .flex_grow(1.0)
                .into_node(),
                Text::new(match_info).size(11.0).color(dim).into_node(),
                small_btn("^", find_prev),
                small_btn("v", find_next),
            ],
        }.into_node();

        // Replace row
        let replace_row = HStack {
            spacing: Some(4.0),
            children: vec![
                Container::new(
                    TextInput {
                        id: Some(fission_ir::NodeId::explicit("editor_replace_query_input")),
                        value: view.state.replace_query.clone(),
                        placeholder: Some("Replace".into()),
                        on_change: Some(update_replace),
                        borderless: true,
                        ..Default::default()
                    }.into_node(),
                )
                .bg(Color { r: 60, g: 60, b: 60, a: 255 })
                .border(border, 1.0)
                .border_radius(2.0)
                .height(24.0)
                .flex_grow(1.0)
                .into_node(),
                small_btn("Replace", replace_one),
                small_btn("All", replace_all),
            ],
        }.into_node();

        Container::new(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Container::new(
                        fission_widgets::VStack {
                            spacing: Some(4.0),
                            children: vec![find_row, replace_row],
                        }.into_node(),
                    ).flex_grow(1.0).into_node(),
                    small_btn("x", close),
                ],
            }.into_node(),
        )
        .bg(bg)
        .height(60.0)
        .padding_all(6.0)
        .border(Color { r: 48, g: 48, b: 49, a: 255 }, 1.0)
        .flex_shrink(0.0)
        .into_node()
    }
}
