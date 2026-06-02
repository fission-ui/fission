use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Text, Widget};
use fission_core::{
    build::{BuildCtxHandle, ViewHandle},
    ActionEnvelope,
};
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

impl From<TreeView> for Widget {
    fn from(component: TreeView) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let this = &component;

        let mut nodes = Vec::new();
        for item in &this.items {
            this.build_recursive(item, 0, &mut nodes, &ctx, view);
        }

        VStack {
            spacing: Some(0.0),
            children: nodes,
        }
        .into()
    }
}

impl TreeView {
    fn build_recursive(
        &self,
        item: &TreeItem,
        depth: usize,
        nodes: &mut Vec<Widget>,
        ctx: &BuildCtxHandle<()>,
        view: ViewHandle<()>,
    ) {
        let is_selected = self.selected_id.as_ref() == Some(&item.id);
        let is_expanded = self.expanded_ids.contains(&item.id);

        let theme = &view.env().theme.components.tree_view;
        let tokens = &view.env().theme.tokens;

        let mut row_children = Vec::new();

        // Indentation
        row_children.push(
            fission_core::ui::widgets::Spacer {
                width: Some(depth as f32 * theme.indent),
                ..Default::default()
            }
            .into(),
        );

        // Icon
        if let Some(icon) = &item.icon {
            row_children.push(
                Icon::svg(icon.clone())
                    .size(18.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
            row_children.push(
                fission_core::ui::widgets::Spacer {
                    width: Some(8.0),
                    ..Default::default()
                }
                .into(),
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
                .into(),
        );

        let row_content = Container::new(HStack {
            spacing: Some(0.0),
            children: row_children,
        })
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
        .into();

        nodes.push(
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(row_content),
                on_press: item.on_select.clone(),
                padding: Some([0.0; 4]),
                height: Some(40.0), // Force button height
                ..Default::default()
            }
            .into(),
        );

        if is_expanded {
            for child in &item.children {
                self.build_recursive(child, depth + 1, nodes, ctx, view);
            }
        }
    }
}
