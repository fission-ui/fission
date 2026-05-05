use crate::Icon;
use fission_core::ui::{Button, ButtonVariant, Node, Row, TextInput};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NumberInput {
    pub id: Option<WidgetNodeId>,
    pub value: f32,
    pub display_text: Option<String>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub step: f32,
    pub field_width: Option<f32>,
    pub button_size: Option<f32>,
    pub gap: Option<f32>,
    pub on_increment: Option<ActionEnvelope>,
    pub on_decrement: Option<ActionEnvelope>,
    pub on_change: Option<ActionEnvelope>, // Text input change
}

impl Default for NumberInput {
    fn default() -> Self {
        Self {
            id: None,
            value: 0.0,
            display_text: None,
            min: None,
            max: None,
            step: 1.0,
            field_width: None,
            button_size: None,
            gap: None,
            on_increment: None,
            on_decrement: None,
            on_change: None,
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for NumberInput {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let _tokens = &view.env.theme.tokens;
        let display_text = self
            .display_text
            .clone()
            .unwrap_or_else(|| format!("{}", self.value));
        let field_width = self.field_width.unwrap_or(60.0);
        let button_size = self.button_size.unwrap_or(32.0);
        let input_id = self
            .id
            .as_ref()
            .map(|id| NodeId::derived(id.as_u128(), &[0]));

        Row::default()
            .gap(self.gap.unwrap_or(4.0))
            .align_items(fission_ir::op::AlignItems::Center)
            .children(vec![
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Box::new(
                        Icon::svg(material::content::remove::regular())
                            .size(16.0)
                            .into_node(),
                    )),
                    on_press: self.on_decrement.clone(),
                    width: Some(button_size),
                    height: Some(button_size),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }
                .into_node(),
                TextInput {
                    id: input_id,
                    value: display_text,
                    width: Some(field_width),
                    // TODO: Parse text input back to float for on_change
                    // Needs `on_change` logic similar to slider?
                    // MVP: Just display value.
                    ..Default::default()
                }
                .into_node(),
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Box::new(
                        Icon::svg(material::content::add::regular())
                            .size(16.0)
                            .into_node(),
                    )),
                    on_press: self.on_increment.clone(),
                    width: Some(button_size),
                    height: Some(button_size),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }
                .into_node(),
            ])
            .into_node()
    }
}
