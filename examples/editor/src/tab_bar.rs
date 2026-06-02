use crate::model::{CloseTab, EditorState, SelectTab};
use fission::core::op::Color;
use fission::core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Text, Widget};
use fission::core::{reduce_with, ActionEnvelope};
use fission::widgets::{HStack, Spacer};
use serde_json;

pub struct TabBar;

impl From<TabBar> for Widget {
    fn from(_component: TabBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let _tokens = &view.env().theme.tokens;

        if view.state().open_tabs.is_empty() {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let select_id = ctx
            .bind(
                SelectTab(0),
                reduce_with!(
                    (|s: &mut EditorState, a: SelectTab, _| {
                        s.active_tab = a.0;
                        s.update_breadcrumb();
                    })
                ),
            )
            .id;

        let close_id = ctx
            .bind(
                CloseTab(0),
                reduce_with!(
                    (|s: &mut EditorState, a: CloseTab, _| {
                        s.close_tab(a.0);
                    })
                ),
            )
            .id;

        let mut tab_nodes = Vec::new();
        for (i, tab) in view.state().open_tabs.iter().enumerate() {
            let is_active = i == view.state().active_tab;
            let bg = if is_active {
                Color {
                    r: 30,
                    g: 30,
                    b: 30,
                    a: 255,
                } // Dark editor bg
            } else {
                Color {
                    r: 45,
                    g: 45,
                    b: 45,
                    a: 255,
                } // Slightly lighter
            };
            let text_color = if is_active {
                Color {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                }
            } else {
                Color {
                    r: 160,
                    g: 160,
                    b: 160,
                    a: 255,
                }
            };
            let title = if tab.is_dirty {
                format!("* {}", tab.title)
            } else {
                tab.title.clone()
            };

            let accent_color = Color {
                r: 0,
                g: 122,
                b: 204,
                a: 255,
            };
            let top_border = if is_active {
                Container::new(Spacer {
                    ..Default::default()
                })
                .height(2.0)
                .bg(accent_color)
                .into()
            } else {
                Container::new(Spacer {
                    ..Default::default()
                })
                .height(2.0)
                .bg(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                })
                .into()
            };

            let tab_content: Widget = HStack {
                spacing: Some(6.0),
                children: vec![
                    Text::new(title).size(12.0).color(text_color).into(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(
                            Text::new("×")
                                .size(14.0)
                                .color(Color {
                                    r: 160,
                                    g: 160,
                                    b: 160,
                                    a: 255,
                                })
                                .into(),
                        ),
                        on_press: Some(ActionEnvelope {
                            id: close_id,
                            payload: serde_json::to_vec(&CloseTab(i)).unwrap(),
                        }),
                        width: Some(20.0),
                        height: Some(20.0),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into(),
                ],
            }
            .into();

            let tab_with_accent = fission::core::ui::Column {
                children: vec![
                    top_border,
                    Container::new(tab_content)
                        .bg(bg)
                        .padding_all(6.0)
                        .flex_grow(1.0)
                        .into(),
                ],
                ..Default::default()
            }
            .into();

            tab_nodes.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(tab_with_accent),
                    on_press: Some(ActionEnvelope {
                        id: select_id,
                        payload: serde_json::to_vec(&SelectTab(i)).unwrap(),
                    }),
                    height: Some(35.0),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }
                .into(),
            );
        }

        Container::new(fission::core::ui::Scroll {
            direction: fission::op::FlexDirection::Row,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 0.0,
            child: Some(
                HStack {
                    spacing: Some(0.0),
                    children: tab_nodes,
                }
                .into(),
            ),
            ..Default::default()
        })
        .height(35.0)
        .min_height(35.0)
        .bg(Color {
            r: 37,
            g: 37,
            b: 38,
            a: 255,
        })
        .border(
            Color {
                r: 48,
                g: 48,
                b: 49,
                a: 255,
            },
            1.0,
        )
        .flex_shrink(0.0)
        .into()
    }
}
