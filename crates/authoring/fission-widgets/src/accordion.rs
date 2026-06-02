use crate::stack::{HStack, VStack};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Text, TextContent, Widget,
};
use fission_core::ActionEnvelope;
use serde::{Deserialize, Serialize};

/// A single collapsible section within an [`Accordion`].
///
/// When `is_expanded` is `true`, the content is visible below the header.
/// The header displays a chevron indicator (triangledown/triangleright) and the title text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccordionItem {
    pub title: String,
    pub content: Widget,
    pub is_expanded: bool,
    pub on_toggle: Option<ActionEnvelope>,
}

/// A vertical list of collapsible sections.
///
/// Each [`AccordionItem`] has a clickable header that toggles its content visibility.
/// Items are stacked with zero gap, creating a continuous bordered surface.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Accordion {
    pub items: Vec<AccordionItem>,
}

impl From<Accordion> for Widget {
    fn from(component: Accordion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let mut children = Vec::new();

        for item in &this.items {
            // Header
            children.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(
                        Container::new(HStack {
                            spacing: Some(8.0),
                            children: vec![
                                // Expand icon (chevron)
                                Text {
                                    content: TextContent::Literal(
                                        if item.is_expanded { "▼" } else { "▶" }.into(),
                                    ),
                                    font_size: Some(12.0),
                                    color: Some(tokens.colors.text_secondary),
                                    ..Default::default()
                                }
                                .into(),
                                // Title
                                Text {
                                    content: TextContent::Literal(item.title.clone()),
                                    color: Some(tokens.colors.text_primary),
                                    flex_grow: 1.0,
                                    ..Default::default()
                                }
                                .into(),
                            ],
                        })
                        .padding_all(tokens.spacing.m)
                        .bg(tokens.colors.surface)
                        .border(tokens.colors.border, 1.0)
                        .into(),
                    ),
                    on_press: item.on_toggle.clone(),
                    ..Default::default()
                }
                .into(),
            );

            // Content
            if item.is_expanded {
                children.push(
                    Container::new(item.content.clone())
                        .padding_all(tokens.spacing.m)
                        .bg(tokens.colors.background)
                        .border(tokens.colors.border, 1.0)
                        .into(),
                );
            }
        }

        VStack {
            spacing: Some(0.0), // No gap between items
            children,
        }
        .into()
    }
}
