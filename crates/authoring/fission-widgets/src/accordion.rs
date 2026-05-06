use crate::stack::{HStack, VStack};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Node, Text, TextContent,
};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

/// A single collapsible section within an [`Accordion`].
///
/// When `is_expanded` is `true`, the content is visible below the header.
/// The header displays a chevron indicator (triangledown/triangleright) and the title text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccordionItem {
    pub title: String,
    pub content: Node,
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

impl<S: fission_core::AppState> Widget<S> for Accordion {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;

        let mut children = Vec::new();

        for item in &self.items {
            // Header
            children.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(Box::new(
                        Container::new(
                            HStack {
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
                            }
                            .build(ctx, view),
                        )
                        .padding_all(tokens.spacing.m)
                        .bg(tokens.colors.surface)
                        .border(tokens.colors.border, 1.0)
                        .into_node(),
                    )),
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
                        .into_node(),
                );
            }
        }

        VStack {
            spacing: Some(0.0), // No gap between items
            children,
        }
        .build(ctx, view)
    }
}
