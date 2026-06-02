use crate::stack::VStack;
use fission_core::ui::{Container, Text, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineItem {
    pub title: String,
    pub description: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    pub items: Vec<TimelineItem>,
}

impl From<Timeline> for Widget {
    fn from(component: Timeline) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.timeline;
        let tokens = &view.env().theme.tokens;
        let mut children = Vec::new();

        for (i, item) in this.items.iter().enumerate() {
            let is_last = i == this.items.len() - 1;

            let marker: Widget = VStack {
                spacing: Some(0.0),
                children: vec![
                    // Dot
                    Container::new(fission_core::ui::widgets::Spacer::default())
                        .width(theme.dot_size)
                        .height(theme.dot_size)
                        .border_radius(theme.dot_size / 2.0)
                        .bg(theme.dot_color)
                        .into(),
                    // Line
                    if !is_last {
                        Container::new(fission_core::ui::widgets::Spacer::default())
                            .width(theme.line_width)
                            .flex_grow(1.0)
                            .bg(theme.line_color)
                            .into()
                    } else {
                        fission_core::ui::widgets::Spacer::default().into()
                    },
                ],
            }
            .into();

            // Content
            let mut content_children = vec![Text::new(item.title.clone())
                .size(tokens.typography.body_large_size)
                .color(tokens.colors.text_primary)
                .into()];

            if let Some(ts) = &item.timestamp {
                content_children.push(
                    Text::new(ts.clone())
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }

            if let Some(desc) = &item.description {
                content_children.push(
                    Text::new(desc.clone())
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }

            let content = Container::new(VStack {
                spacing: Some(4.0),
                children: content_children,
            })
            .padding_all(0.0) // padding-bottom handled by item spacing?
            .flex_grow(1.0)
            .into();

            // Item Row
            // We need to constrain marker width.
            use fission_core::ui::Row;
            children.push(
                Row {
                    children: vec![Container::new(marker).width(20.0).into(), content],
                    // Align items start?
                    ..Default::default()
                }
                .into(),
            );

            // Spacing between items
            if !is_last {
                children.push(
                    fission_core::ui::widgets::Spacer {
                        height: Some(16.0),
                        ..Default::default()
                    }
                    .into(),
                );
            }
        }

        VStack {
            spacing: Some(0.0),
            children,
        }
        .into()
    }
}
