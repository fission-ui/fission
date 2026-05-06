use crate::stack::VStack;
use crate::Icon;
use fission_core::op::Color;
use fission_core::ui::{Container, Node, Row, Text};
use fission_core::{BuildCtx, View, Widget};
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AlertKind {
    Info,
    Warning,
    Error,
    Success,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub kind: AlertKind,
    pub title: String,
    pub description: Option<String>,
}

impl<S: fission_core::AppState> Widget<S> for Alert {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.alert;
        let tokens = &view.env.theme.tokens;

        let (bg, icon, color) = match self.kind {
            AlertKind::Info => (
                theme.info_bg,
                material::action::info::regular(),
                Color::BLUE,
            ),
            AlertKind::Warning => (
                theme.warning_bg,
                material::alert::warning::regular(),
                Color {
                    r: 255,
                    g: 165,
                    b: 0,
                    a: 255,
                },
            ),
            AlertKind::Error => (
                theme.error_bg,
                material::alert::error::regular(),
                Color::RED,
            ),
            AlertKind::Success => (
                theme.success_bg,
                material::action::check_circle::regular(),
                Color::GREEN,
            ),
        };

        Container::new(
            Row {
                gap: Some(12.0),
                align_items: fission_ir::op::AlignItems::Center,
                children: vec![
                    Icon::svg(icon).size(24.0).color(color).into_node(),
                    Container::new(
                        VStack {
                            spacing: Some(2.0),
                            children: vec![
                                Text::new(self.title.clone())
                                    .size(tokens.typography.body_large_size)
                                    .into_node(),
                                if let Some(desc) = &self.description {
                                    Text::new(desc.clone())
                                        .size(tokens.typography.body_medium_size)
                                        .color(tokens.colors.text_secondary)
                                        .into_node()
                                } else {
                                    fission_core::ui::widgets::spacer::Spacer::default().into_node()
                                },
                            ],
                        }
                        .into_node(),
                    )
                    .flex_grow(1.0)
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .bg(bg)
        .padding_all(12.0)
        .border_radius(theme.radius)
        .into_node()
    }
}
