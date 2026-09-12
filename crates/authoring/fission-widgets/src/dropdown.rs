use crate::stack::HStack;
use crate::Icon;
use fission_core::action::ActionEnvelope;
use fission_core::ui::{Button, ButtonContentAlign, Text, TextContent};
use fission_core::Widget;
use fission_icons::material;
use fission_ir::{PopupKind, Role, Semantics};

/// A simplified dropdown trigger button.
///
/// Renders as an outline button with the selected value (or "Select an option")
/// and a chevron icon. This is a trigger-only widget -- it does not render the
/// dropdown list itself. Use [`Select`](crate::Select) for a complete dropdown
/// with a popup menu.
#[derive(Default, Clone)]
pub struct DropDown {
    /// Action dispatched when the trigger is pressed.
    pub on_toggle: Option<ActionEnvelope>,
    /// Candidate labels retained for compatibility with existing dropdown
    /// models; this trigger-only widget does not render them.
    pub options: Vec<String>,
    /// Selection action retained for compatibility; use [`crate::Select`] when
    /// the widget itself should present and dispatch option selection.
    pub on_select: Option<ActionEnvelope>,
    /// Label displayed in the trigger, or the default placeholder when absent.
    pub selected: Option<String>,
    /// Whether the menu this trigger controls is currently open.
    ///
    /// Reported to assistive technology. A trigger that never says it is open
    /// leaves a reader unable to tell whether pressing it did anything.
    pub is_open: bool,
}

impl From<DropDown> for Widget {
    fn from(component: DropDown) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let button_text = this.selected.as_deref().unwrap_or("Select an option");
        let tokens = &view.env().theme.tokens;
        let recipe = view.env().theme.recipe("dropdown");
        let value_style = recipe.part("value");
        let indicator_style = recipe.part("indicator");

        Button {
            variant: fission_core::ui::ButtonVariant::Outline,
            child: Some(
                HStack {
                    spacing: Some(recipe.base.gap.unwrap_or(tokens.spacing.s)),
                    children: vec![
                        Text {
                            content: TextContent::Literal(button_text.into()),
                            font_size: Some(
                                value_style
                                    .font_size
                                    .unwrap_or(tokens.typography.body_medium_size),
                            ),
                            color: Some(tokens.colors.text_primary),
                            ..Default::default()
                        }
                        .into(),
                        Icon::svg(material::navigation::expand_more::regular())
                            .size(indicator_style.icon_size.unwrap_or(18.0))
                            .color(
                                indicator_style
                                    .text_color
                                    .unwrap_or(tokens.colors.text_secondary),
                            )
                            .into(),
                    ],
                }
                .into(),
            ),
            on_press: this.on_toggle.clone(),
            content_align: ButtonContentAlign::Start,
            padding: Some([tokens.spacing.s, tokens.spacing.s, 0.0, 0.0]),
            semantics: Some(Semantics {
                role: Role::ComboBox,
                label: Some(button_text.to_string()),
                value: Some(button_text.to_string()),
                // A trigger that opens a menu has to announce that it does, and
                // whether it is currently open, or a reader has no way to know
                // pressing it reveals anything.
                has_popup: Some(PopupKind::Menu),
                expanded: Some(this.is_open),
                focusable: true,
                ..Default::default()
            }),
            ..Default::default()
        }
        .into()
    }
}
