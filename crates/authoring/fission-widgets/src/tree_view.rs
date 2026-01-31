use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Node, Text, TextContent};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use fission_core::op::Color;
use crate::stack::{VStack, HStack};
use crate::Icon;
use fission_icons::material;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
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
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let mut nodes = Vec::new();
        for item in &self.items {
            self.build_recursive(item, 0, &mut nodes, ctx, view);
        }

        crate::stack::VStack {
            spacing: Some(0.0),
            children: nodes,
        }.build(ctx, view)
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
        let theme = &view.env.theme.components.tree_view;
        let tokens = &view.env.theme.tokens;
        let is_expanded = self.expanded_ids.contains(&item.id);
        let is_selected = self.selected_id.as_ref() == Some(&item.id);
        let has_children = !item.children.is_empty();

        let mut row_children = Vec::new();
        
        // Indentation
        row_children.push(
            fission_core::ui::widgets::Spacer { width: Some(depth as f32 * theme.indent), ..Default::default() }.into_node()
        );

        // Chevron
        let chevron_icon = if is_expanded {
            material::navigation::expand_more::regular()
        } else {
            material::navigation::chevron_right::regular()
        };
        
        if has_children {
             row_children.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(Icon::svg(chevron_icon).size(16.0).color(tokens.colors.text_secondary).into_node())),
                    on_press: item.on_toggle.clone(),
                    width: Some(20.0), height: Some(20.0),
                    ..Default::default()
                }.into_node()
            );
        } else {
             row_children.push(fission_core::ui::widgets::Spacer { width: Some(20.0), ..Default::default() }.into_node());
        }

        // Icon
        if let Some(icon) = &item.icon {
            row_children.push(Icon::svg(icon.clone()).size(18.0).color(tokens.colors.text_secondary).into_node());
            row_children.push(fission_core::ui::widgets::Spacer { width: Some(8.0), ..Default::default() }.into_node());
        }

        // Label
        row_children.push(
            Text::new(item.label.clone())
                .color(if is_selected { tokens.colors.primary } else { tokens.colors.text_primary })
                .flex_grow(1.0)
                .into_node()
        );

        let row_content = Container::new(
            HStack {
                spacing: Some(0.0),
                children: row_children,
            }.into_node()
        )
        .bg(if is_selected { theme.selected_bg } else { fission_core::op::Color { r:0,g:0,b:0,a:0 } })
        .padding_all(8.0)
        .height(36.0)
        .flex_grow(1.0)
        .into_node();
        
        nodes.push(
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(Box::new(row_content)),
                on_press: item.on_select.clone(),
                ..Default::default()
            }.into_node()
        );

        if is_expanded {
            for child in &item.children {
                self.build_recursive(child, depth + 1, nodes, ctx, view);
            }
        }
    }
}
