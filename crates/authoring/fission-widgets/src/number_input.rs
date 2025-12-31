use fission_core::ui::{Button, ButtonVariant, Container, Node, Text, TextContent, TextInput, Row};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId};
use crate::Icon;
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NumberInput {
    pub id: Option<WidgetNodeId>,
    pub value: f32,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub step: f32,
    pub on_increment: Option<ActionEnvelope>,
    pub on_decrement: Option<ActionEnvelope>,
    pub on_change: Option<ActionEnvelope>, // Text input change
}

impl Default for NumberInput {
    fn default() -> Self {
        Self {
            id: None,
            value: 0.0,
            min: None,
            max: None,
            step: 1.0,
            on_increment: None,
            on_decrement: None,
            on_change: None,
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for NumberInput {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        Row::default()
            .gap(4.0)
            .align_items(fission_ir::op::AlignItems::Center)
            .children(vec![
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Box::new(Icon::svg(material::content::remove::regular()).size(16.0).into_node())),
                    on_press: self.on_decrement.clone(),
                    width: Some(32.0),
                    height: Some(32.0),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }.into_node(),
                
                TextInput {
                    value: format!("{}", self.value),
                    width: Some(60.0),
                    // TODO: Parse text input back to float for on_change
                    // Needs `on_change` logic similar to slider? 
                    // MVP: Just display value.
                    ..Default::default()
                }.into_node(),
                
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Box::new(Icon::svg(material::content::add::regular()).size(16.0).into_node())),
                    on_press: self.on_increment.clone(),
                    width: Some(32.0),
                    height: Some(32.0),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }.into_node(),
            ])
        .into_node()
    }
}
