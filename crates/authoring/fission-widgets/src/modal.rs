use fission_core::ui::{Button, ButtonVariant, Container, Node, Text, TextContent, ZStack};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId, NodeId};
use fission_core::op::{Color, BoxShadow};
use crate::stack::{VStack, HStack};
use crate::{Icon, Portal};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Modal {
    pub id: WidgetNodeId,
    pub title: String,
    pub content: Box<Node>,
    pub is_open: bool,
    pub on_dismiss: Option<ActionEnvelope>,
    pub actions: Vec<ModalAction>,
    pub width: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalAction {
    pub label: String,
    pub on_press: Option<ActionEnvelope>,
    pub is_primary: bool,
}

impl<S: fission_core::AppState> Widget<S> for Modal {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        if !self.is_open {
            return fission_core::ui::widgets::spacer::Spacer::default().into_node();
        }

        let theme = &view.env.theme.components.modal;
        let tokens = &view.env.theme.tokens;

        // Dimmed backdrop
        let backdrop = Container::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
            .bg(Color { r: 0, g: 0, b: 0, a: 128 }) 
            .into_node();
        
        let backdrop_btn = Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(backdrop)),
            on_press: self.on_dismiss.clone(),
            ..Default::default()
        }.into_node();

        // Modal Content
        let mut action_buttons = Vec::new();
        for action in &self.actions {
            action_buttons.push(
                Button {
                    variant: if action.is_primary { ButtonVariant::Filled } else { ButtonVariant::Outline },
                    child: Some(Box::new(Text::new(action.label.clone())
                        .color(if action.is_primary { tokens.colors.on_primary } else { tokens.colors.primary })
                        .into_node())),
                    on_press: action.on_press.clone(),
                    ..Default::default()
                }.into_node()
            );
        }

        let mut modal_card_builder = Container::new(
            VStack {
                spacing: Some(16.0),
                children: vec![
                    // Header
                    HStack {
                        spacing: Some(8.0),
                        children: vec![
                            Text::new(self.title.clone())
                                .size(20.0)
                                .into_node(),
                            fission_core::ui::widgets::spacer::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                            Button {
                                variant: ButtonVariant::Ghost,
                                child: Some(Box::new(Icon::svg(fission_icons::material::navigation::close::regular()).size(20.0).into_node())),
                                on_press: self.on_dismiss.clone(),
                                ..Default::default()
                            }.into_node(),
                        ]
                    }.into_node(),
                    
                    // Content
                    *self.content.clone(),
                    
                    // Footer Actions
                    HStack {
                        spacing: Some(8.0),
                        children: vec![
                            fission_core::ui::widgets::spacer::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                        ].into_iter().chain(action_buttons).collect(),
                    }.into_node(),
                ]
            }.into_node()
        )
        .bg(theme.bg_color)
        .border_radius(theme.radius);
        
        if let Some(s) = theme.shadow {
            modal_card_builder = modal_card_builder.shadow(s);
        }
        
        let modal_card = modal_card_builder
            .width(self.width.unwrap_or(theme.max_width))
            .padding_all(24.0)
            .into_node();

        let root = Container::new(
            ZStack {
                children: vec![
                    fission_core::ui::Positioned {
                        left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                        child: Some(Box::new(backdrop_btn)),
                        ..Default::default()
                    }.into_node(),
                    modal_card
                ],
                ..Default::default()
            }.into_node()
        )
        .into_node();
        
        let positioned_root = fission_core::ui::Positioned {
            left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
            child: Some(Box::new(root)),
            ..Default::default()
        }.into_node();

        ctx.register_portal(positioned_root);

        fission_core::ui::widgets::spacer::Spacer::default().into_node()
    }
}