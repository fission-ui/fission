use fission_core::ui::{Container, Node, Text, Row, Button, ButtonVariant};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId};
use fission_core::op::Color;
use crate::stack::HStack;
use crate::Icon;
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toast {
    pub id: WidgetNodeId,
    pub kind: ToastKind,
    pub message: String,
    pub on_close: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Toast {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        let (icon_path, icon_color) = match self.kind {
            ToastKind::Info => (material::action::info::regular(), tokens.colors.primary),
            ToastKind::Success => (material::action::check_circle::regular(), tokens.colors.on_background), 
            ToastKind::Warning => (material::action::report_problem::regular(), Color { r: 255, g: 152, b: 0, a: 255 }),
            ToastKind::Error => (material::alert::error::regular(), tokens.colors.error),
        };

        let content = HStack {
            spacing: Some(12.0),
            children: vec![
                Icon::svg(icon_path).color(icon_color).size(20.0).into_node(),
                Text::new(self.message.clone())
                    .color(tokens.colors.on_surface)
                    .flex_grow(1.0)
                    .into_node(),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(Icon::svg(material::navigation::close::regular()).size(16.0).into_node())),
                    on_press: self.on_close.clone(),
                    ..Default::default()
                }.into()
            ]
        }.into_node();

        Container::new(content)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.medium)
            .shadow(tokens.elevations.level3.unwrap_or(fission_core::op::BoxShadow {
                color: Color { r: 0, g: 0, b: 0, a: 60 },
                blur_radius: 12.0,
                offset: (0.0, 6.0),
            }))
            .padding_all(12.0)
            .into_node()
    }
}
