use crate::stack::VStack;
use crate::Icon;
use fission_core::op::Color;
use fission_core::ui::{Container, Row, Text, Widget};
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

impl From<Alert> for Widget {
    fn from(component: Alert) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.alert;
        let tokens = &view.env().theme.tokens;

        let (bg, icon, color) = match this.kind {
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

        Container::new(Row {
            gap: Some(12.0),
            align_items: fission_ir::op::AlignItems::Center,
            children: vec![
                Icon::svg(icon).size(24.0).color(color).into(),
                Container::new(VStack {
                    spacing: Some(2.0),
                    children: vec![
                        Text::new(this.title.clone())
                            .size(tokens.typography.body_large_size)
                            .into(),
                        if let Some(desc) = &this.description {
                            Text::new(desc.clone())
                                .size(tokens.typography.body_medium_size)
                                .color(tokens.colors.text_secondary)
                                .into()
                        } else {
                            fission_core::ui::widgets::spacer::Spacer::default().into()
                        },
                    ],
                })
                .flex_grow(1.0)
                .into(),
            ],
            ..Default::default()
        })
        .bg(bg)
        .padding_all(12.0)
        .border_radius(theme.radius)
        .into()
    }
}
