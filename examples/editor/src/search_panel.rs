use crate::model::{EditorState, ExecuteSearch, OpenFile, UpdateSearchQuery};
use fission::core::op::Color;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Column, Container, Scroll, Text, TextInput,
};
use fission::core::{reduce_with, ActionEnvelope, BuildCtx, FlexDirection, View, Widget};
use fission::widgets::{HStack, VStack};
use serde_json;

pub struct SearchPanel;

impl Widget<EditorState> for SearchPanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
            let text_color = Color {
                r: 204,
                g: 204,
                b: 204,
                a: 255,
            };
            let dim_color = Color {
                r: 140,
                g: 140,
                b: 140,
                a: 255,
            };

            let update_query = ctx.bind(
                UpdateSearchQuery(String::new()),
                reduce_with!((|s: &mut EditorState, a: UpdateSearchQuery, _| s.search_query = a.0)),
            );

            let execute = ctx.bind(
                ExecuteSearch,
                reduce_with!((|s: &mut EditorState, _, _| s.run_search())),
            );

            let open_id = ctx
                .bind(
                    OpenFile(String::new()),
                    reduce_with!((|s: &mut EditorState, a: OpenFile, _| s.open_file(a.0))),
                )
                .id;

            // Connected search input with Go button inside a single bordered container
            let search_row = Container::new(
                HStack {
                    spacing: Some(0.0),
                    children: vec![
                        TextInput {
                            id: Some(fission::ir::NodeId::explicit("editor_search_query_input")),
                            value: view.state.search_query.clone(),
                            placeholder: Some("Search...".into()),
                            on_change: Some(update_query),
                            borderless: true,
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Box::new(
                                Text::new("Go").size(11.0).color(text_color).into_node(),
                            )),
                            on_press: Some(execute),
                            width: Some(32.0),
                            height: Some(28.0),
                            padding: Some([0.0; 4]),
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                }
                .into_node(),
            )
            .bg(Color {
                r: 60,
                g: 60,
                b: 60,
                a: 255,
            })
            .border(
                Color {
                    r: 80,
                    g: 80,
                    b: 80,
                    a: 255,
                },
                1.0,
            )
            .border_radius(3.0)
            .height(30.0)
            .into_node();

            let mut children = vec![search_row];

            // Results
            if !view.state.search_results.is_empty() {
                children.push(
                    Text::new(format!("{} results", view.state.search_results.len()))
                        .size(11.0)
                        .color(dim_color)
                        .into_node(),
                );

                let mut result_nodes = Vec::new();
                for result in view.state.search_results.iter().take(50) {
                    let label = format!(
                        "{}:{}",
                        result.path.rsplit('/').next().unwrap_or(&result.path),
                        result.line
                    );
                    result_nodes.push(
                        Button {
                            variant: ButtonVariant::Ghost,
                            content_align: ButtonContentAlign::Start,
                            child: Some(Box::new(
                                VStack {
                                    spacing: Some(1.0),
                                    children: vec![
                                        Text::new(label).size(12.0).color(text_color).into_node(),
                                        Text::new(
                                            result.context.chars().take(60).collect::<String>(),
                                        )
                                        .size(11.0)
                                        .color(dim_color)
                                        .into_node(),
                                    ],
                                }
                                .into_node(),
                            )),
                            on_press: Some(ActionEnvelope {
                                id: open_id,
                                payload: serde_json::to_vec(&OpenFile(result.path.clone()))
                                    .unwrap(),
                            }),
                            padding: Some([4.0, 4.0, 0.0, 0.0]),
                            ..Default::default()
                        }
                        .into_node(),
                    );
                }

                children.push(
                    Scroll {
                        direction: FlexDirection::Column,
                        child: Some(Box::new(
                            VStack {
                                spacing: Some(2.0),
                                children: result_nodes,
                            }
                            .into_node(),
                        )),
                        show_scrollbar: true,
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                );
            } else if !view.state.search_query.is_empty() {
                children.push(
                    Text::new("No results found")
                        .size(12.0)
                        .color(dim_color)
                        .into_node(),
                );
            }

            Container::new(
                Column {
                    gap: Some(8.0),
                    children,
                    flex_grow: 1.0,
                    justify_content: fission::core::op::JustifyContent::Start,
                    ..Default::default()
                }
                .into_node(),
            )
            .padding_all(8.0)
            .flex_grow(1.0)
            .into_node()
        })
    }
}
