use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Node, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeItem {
    pub id: String,
    pub icon: Option<String>,
    pub label: String,
    pub children: Vec<TreeItem>,
    pub on_toggle: Option<ActionEnvelope>,
    pub on_select: Option<ActionEnvelope>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeView {
    pub items: Vec<TreeItem>,
    pub expanded_ids: HashSet<String>,
    pub selected_id: Option<String>,
}

impl<S: fission_core::AppState> Widget<S> for TreeView {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let mut nodes = Vec::new();
            for item in &self.items {
                self.build_recursive(item, 0, &mut nodes, ctx, view);
            }

            VStack {
                spacing: Some(0.0),
                children: nodes,
            }
            .build_node(ctx, view)
        })
    }
}

impl TreeView {
    fn build_recursive<S: fission_core::AppState>(
        &self,
        item: &TreeItem,
        depth: usize,
        nodes: &mut Vec<Node>,
        ctx: &mut BuildCtx<S>,
        view: &View<S>,
    ) {
        let is_selected = self.selected_id.as_ref() == Some(&item.id);
        let is_expanded = self.expanded_ids.contains(&item.id);

        let theme = &view.env.theme.components.tree_view;
        let tokens = &view.env.theme.tokens;

        let mut row_children = Vec::new();

        // Indentation
        row_children.push(
            fission_core::ui::widgets::Spacer {
                width: Some(depth as f32 * theme.indent),
                ..Default::default()
            }
            .into_node(),
        );

        // Icon
        if let Some(icon) = &item.icon {
            row_children.push(
                Icon::svg(icon.clone())
                    .size(18.0)
                    .color(tokens.colors.text_secondary)
                    .into_node(),
            );
            row_children.push(
                fission_core::ui::widgets::Spacer {
                    width: Some(8.0),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        // Label
        row_children.push(
            Text::new(item.label.clone())
                .size(15.0)
                .color(if is_selected {
                    tokens.colors.primary
                } else {
                    tokens.colors.text_primary
                })
                .flex_grow(1.0)
                .into_node(),
        );

        let row_content = Container::new(
            HStack {
                spacing: Some(0.0),
                children: row_children,
            }
            .into_node(),
        )
        .padding_all(8.0)
        .height(40.0)
        .bg(if is_selected {
            theme.selected_bg
        } else {
            fission_core::op::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            }
        })
        .border_radius(tokens.radii.medium)
        .flex_grow(1.0)
        .into_node();

        nodes.push(
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(Box::new(row_content)),
                on_press: item.on_select.clone(),
                padding: Some([0.0; 4]),
                height: Some(40.0), // Force button height
                ..Default::default()
            }
            .into_node(),
        );

        if is_expanded {
            for child in &item.children {
                self.build_recursive(child, depth + 1, nodes, ctx, view);
            }
        }
    }
}
