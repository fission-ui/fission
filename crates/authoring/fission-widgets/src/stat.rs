use crate::stack::VStack;
use fission_core::ui::{Container, Text, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stat {
    pub label: String,
    pub value: String,
    pub help_text: Option<String>,
}

impl From<Stat> for Widget {
    fn from(component: Stat) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let mut children = vec![
            Text::new(this.label.clone())
                .size(13.0)
                .color(tokens.colors.text_secondary)
                .into(),
            Text::new(this.value.clone())
                .size(24.0)
                // .weight(Bold)
                .color(tokens.colors.text_primary)
                .into(),
        ];

        if let Some(help) = &this.help_text {
            children.push(
                Text::new(help.clone())
                    .size(13.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
        }

        Container::new(VStack {
            spacing: Some(4.0),
            children,
        })
        .padding_all(18.0)
        .border(tokens.colors.border, 1.0)
        .border_radius(8.0)
        .into()
    }
}
