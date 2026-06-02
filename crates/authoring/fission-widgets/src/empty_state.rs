use crate::center::Center;
use crate::stack::VStack;
use fission_core::ui::{Container, Text, Widget};
use serde::{Deserialize, Serialize};

/// A centered placeholder displayed when a view has no content.
///
/// Shows an optional icon, a title, an optional description, and an optional
/// action button (e.g., "Create new item"). The entire block is centered in its
/// parent using [`Center`](crate::Center).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyState {
    pub icon: Option<Widget>,
    pub title: String,
    pub description: Option<String>,
    pub action: Option<Widget>,
}

impl From<EmptyState> for Widget {
    fn from(component: EmptyState) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let mut children = Vec::new();

        if let Some(icon) = &this.icon {
            children.push(icon.clone());
        }

        children.push(
            Text::new(this.title.clone())
                .size(tokens.typography.heading_size)
                .color(tokens.colors.text_primary)
                .into(),
        );

        if let Some(desc) = &this.description {
            children.push(
                Text::new(desc.clone())
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
        }

        if let Some(act) = &this.action {
            children.push(
                fission_core::ui::widgets::Spacer {
                    height: Some(16.0),
                    ..Default::default()
                }
                .into(),
            );
            children.push(act.clone());
        }

        Center {
            child: Container::new(VStack {
                spacing: Some(8.0),
                children,
            })
            .padding_all(32.0)
            .into(),
        }
        .into()
    }
}
