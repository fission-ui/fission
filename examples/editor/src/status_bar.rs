use crate::model::EditorState;
use fission_core::op::Color;
use fission_core::ui::{Container, Icon, Node, Text};
use fission_core::{BuildCtx, View, Widget};
use fission_icons::material;
use fission_widgets::{HStack, Spacer};

pub struct StatusBar;

impl Widget<EditorState> for StatusBar {
    fn build(&self, _ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let bg = Color {
            r: 37,
            g: 37,
            b: 38,
            a: 255,
        }; // Workspace dark gray
        let text_color = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };

        let error_color = Color {
            r: 255,
            g: 80,
            b: 80,
            a: 255,
        };
        let warn_color = Color {
            r: 255,
            g: 200,
            b: 60,
            a: 255,
        };

        let mut items = vec![];

        // Branch indicator with icon
        items.push(
            HStack {
                spacing: Some(4.0),
                children: vec![
                    Icon::svg(material::notification::account_tree::round())
                        .size(14.0)
                        .color(text_color)
                        .into_node(),
                    Text::new("main").size(12.0).color(text_color).into_node(),
                ],
            }
            .into_node(),
        );

        items.push(
            Spacer {
                width: Some(16.0),
                ..Default::default()
            }
            .into_node(),
        );

        // Diagnostics summary
        let error_count: usize = view
            .state
            .diagnostics
            .values()
            .flat_map(|d| d.iter())
            .filter(|d| d.severity == crate::model::DiagSeverity::Error)
            .count();
        let warn_count: usize = view
            .state
            .diagnostics
            .values()
            .flat_map(|d| d.iter())
            .filter(|d| d.severity == crate::model::DiagSeverity::Warning)
            .count();

        items.push(
            HStack {
                spacing: Some(4.0),
                children: vec![
                    Icon::svg(material::alert::error::round())
                        .size(14.0)
                        .color(if error_count > 0 {
                            error_color
                        } else {
                            text_color
                        })
                        .into_node(),
                    Text::new(error_count.to_string())
                        .size(12.0)
                        .color(text_color)
                        .into_node(),
                ],
            }
            .into_node(),
        );
        items.push(
            Spacer {
                width: Some(8.0),
                ..Default::default()
            }
            .into_node(),
        );
        items.push(
            HStack {
                spacing: Some(4.0),
                children: vec![
                    Icon::svg(material::alert::warning::round())
                        .size(14.0)
                        .color(if warn_count > 0 {
                            warn_color
                        } else {
                            text_color
                        })
                        .into_node(),
                    Text::new(warn_count.to_string())
                        .size(12.0)
                        .color(text_color)
                        .into_node(),
                ],
            }
            .into_node(),
        );

        items.push(
            Spacer {
                width: Some(16.0),
                ..Default::default()
            }
            .into_node(),
        );

        // Active file info
        if let Some((_tab, buf)) = view.state.active_buffer() {
            items.push(
                Text::new(format!(
                    "Ln {}, Col {}",
                    buf.cursor_line + 1,
                    buf.cursor_col + 1
                ))
                .size(12.0)
                .color(text_color)
                .into_node(),
            );

            items.push(
                Spacer {
                    width: Some(16.0),
                    ..Default::default()
                }
                .into_node(),
            );

            items.push(
                Text::new(buf.language.display_name())
                    .size(12.0)
                    .color(text_color)
                    .into_node(),
            );

            items.push(
                Spacer {
                    width: Some(16.0),
                    ..Default::default()
                }
                .into_node(),
            );

            items.push(Text::new("UTF-8").size(12.0).color(text_color).into_node());

            items.push(
                Spacer {
                    width: Some(16.0),
                    ..Default::default()
                }
                .into_node(),
            );

            items.push(
                Text::new("Spaces: 4")
                    .size(12.0)
                    .color(text_color)
                    .into_node(),
            );
        }

        items.push(
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        );

        // Status message
        if let Some(msg) = &view.state.status_message {
            items.push(
                Text::new(msg.clone())
                    .size(12.0)
                    .color(text_color)
                    .into_node(),
            );
        }

        Container::new(
            HStack {
                spacing: Some(0.0),
                children: items,
            }
            .into_node(),
        )
        .bg(bg)
        .height(26.0)
        .padding_all(4.0)
        .flex_shrink(0.0)
        .into_node()
    }
}
