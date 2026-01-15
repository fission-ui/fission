use fission_core::ui::{Button, ButtonVariant, Container, Node, Text};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use crate::stack::{HStack, VStack};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabItem {
    pub title: String,
    pub content: Node,
    pub on_press: Option<ActionEnvelope>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tabs {
    pub active_index: usize,
    pub items: Vec<TabItem>,
}

impl<S: fission_core::AppState> Widget<S> for Tabs {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.tabs;
        let mut tab_buttons = vec![];
        
        for (i, item) in self.items.iter().enumerate() {
            let is_active = i == self.active_index;
            let color = if is_active { theme.active_color } else { theme.inactive_color };
            
            let tab_button = VStack {
                spacing: Some(2.0),
                children: vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new(item.title.clone()).color(color).into_node())),
                        on_press: item.on_press.clone(),
                        ..Default::default()
                    }.into_node(),
                    if is_active {
                        Container::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
                            .height(theme.indicator_height)
                            .bg(theme.active_color)
                            .into_node()
                    } else {
                        fission_core::ui::widgets::spacer::Spacer::default().into_node()
                    }
                ]
            }.into_node();

            tab_buttons.push(
                Container::new(tab_button)
                    .flex_grow(1.0)
                    .into_node()
            );
        }

        let tab_bar = Container::new(
            HStack {
                spacing: Some(16.0),
                children: tab_buttons,
            }.into_node()
        )
        .bg(theme.background)
        .border(theme.divider_color, 1.0)
        .padding_all(4.0)
        .into_node();

        VStack {
            spacing: Some(16.0),
            children: vec![
                tab_bar,
                if let Some(tab) = self.items.get(self.active_index) {
                    tab.content.clone()
                } else {
                    fission_core::ui::widgets::spacer::Spacer::default().into_node()
                }
            ]
        }.into_node()
    }
}
