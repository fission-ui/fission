use crate::stack::HStack;
use crate::Icon;
use fission_core::action::ActionEnvelope;
use fission_core::ui::{Button, ButtonContentAlign, Text, TextContent};
use fission_core::Widget;
use fission_icons::material;

/// A simplified dropdown trigger button.
///
/// Renders as an outline button with the selected value (or "Select an option")
/// and a chevron icon. This is a trigger-only widget -- it does not render the
/// dropdown list itself. Use [`Select`](crate::Select) for a complete dropdown
/// with a popup menu.
#[derive(Default, Clone)]
pub struct DropDown {
    pub on_toggle: Option<ActionEnvelope>,
    pub options: Vec<String>,
    pub on_select: Option<ActionEnvelope>,
    pub selected: Option<String>,
}

impl From<DropDown> for Widget {
    fn from(component: DropDown) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let button_text = this.selected.as_deref().unwrap_or("Select an option");
        let tokens = &view.env().theme.tokens;

        Button {
            variant: fission_core::ui::ButtonVariant::Outline,
            child: Some(
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Text {
                            content: TextContent::Literal(button_text.into()),
                            font_size: Some(14.0),
                            color: Some(tokens.colors.text_primary),
                            ..Default::default()
                        }
                        .into(),
                        Icon::svg(material::navigation::expand_more::regular())
                            .size(18.0)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
            ),
            on_press: this.on_toggle.clone(),
            content_align: ButtonContentAlign::Start,
            height: Some(40.0),
            padding: Some([12.0, 12.0, 0.0, 0.0]),
            ..Default::default()
        }
        .into()
    }
}
