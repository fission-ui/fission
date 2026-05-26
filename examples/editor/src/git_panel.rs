use crate::model::{EditorState, OpenFile, RefreshGitStatus};
use fission::core::op::Color;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Column, Container, Node, Scroll, Text,
};
use fission::core::{reduce_with, ActionEnvelope, BuildCtx, FlexDirection, View, Widget};
use fission::widgets::{HStack, Spacer, VStack};
use serde_json;

pub struct GitPanel;

impl Widget<EditorState> for GitPanel {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
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
        let added_color = Color {
            r: 80,
            g: 200,
            b: 80,
            a: 255,
        };
        let modified_color = Color {
            r: 220,
            g: 180,
            b: 50,
            a: 255,
        };
        let deleted_color = Color {
            r: 220,
            g: 80,
            b: 80,
            a: 255,
        };

        let refresh = ctx.bind(
            RefreshGitStatus,
            reduce_with!((|s: &mut EditorState, _, _| s.refresh_git_status())),
        );

        let open_id = ctx
            .bind(
                OpenFile(String::new()),
                reduce_with!((|s: &mut EditorState, a: OpenFile, _| s.open_file(a.0))),
            )
            .id;

        let mut children = vec![HStack {
            spacing: Some(4.0),
            children: vec![
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(
                        Text::new("Refresh")
                            .size(11.0)
                            .color(text_color)
                            .into_node(),
                    )),
                    on_press: Some(refresh),
                    height: Some(24.0),
                    padding: Some([4.0, 4.0, 0.0, 0.0]),
                    ..Default::default()
                }
                .into_node(),
            ],
        }
        .into_node()];

        if view.state.git_status_lines.is_empty() {
            children.push(
                Text::new("No changes detected.\nClick ↻ to refresh.")
                    .size(12.0)
                    .color(dim_color)
                    .into_node(),
            );
        } else {
            let mut items = Vec::new();
            for entry in &view.state.git_status_lines {
                let status_color = match entry.status.as_str() {
                    "M" => modified_color,
                    "A" => added_color,
                    "D" => deleted_color,
                    "?" | "??" => dim_color,
                    _ => text_color,
                };

                items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        content_align: ButtonContentAlign::Start,
                        child: Some(Box::new(
                            HStack {
                                spacing: Some(6.0),
                                children: vec![
                                    Text::new(entry.status.clone())
                                        .size(12.0)
                                        .color(status_color)
                                        .into_node(),
                                    Text::new(entry.path.rsplit('/').next().unwrap_or(&entry.path))
                                        .size(12.0)
                                        .color(text_color)
                                        .flex_grow(1.0)
                                        .into_node(),
                                ],
                            }
                            .into_node(),
                        )),
                        on_press: Some(ActionEnvelope {
                            id: open_id,
                            payload: serde_json::to_vec(&OpenFile(entry.path.clone())).unwrap(),
                        }),
                        height: Some(24.0),
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
                            spacing: Some(0.0),
                            children: items,
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
        }

        Container::new(
            Column {
                children,
                gap: Some(8.0),
                flex_grow: 1.0,
                justify_content: fission::core::op::JustifyContent::Start,
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(8.0)
        .bg(Color {
            r: 37,
            g: 37,
            b: 38,
            a: 255,
        }) // Surface background
        .flex_grow(1.0)
        .into_node()
    }
}
