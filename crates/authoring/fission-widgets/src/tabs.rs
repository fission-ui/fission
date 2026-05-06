use crate::stack::{HStack, VStack};
use fission_core::ui::{Button, ButtonVariant, Container, Node, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

/// A single tab definition containing a title, content node, and selection action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabItem {
    pub title: String,
    pub content: Node,
    pub on_press: Option<ActionEnvelope>,
}

/// A tab bar with an active indicator and swappable content area.
///
/// The tab bar displays a horizontal row of tab buttons. The active tab shows
/// a colored indicator bar below its label. The content area below the tab bar
/// displays the `content` node of the tab at `active_index`.
///
/// # Example
///
/// ```rust,ignore
/// Tabs {
///     active_index: 0,
///     items: vec![
///         TabItem { title: "General".into(), content: general_view, on_press: Some(tab0) },
///         TabItem { title: "Advanced".into(), content: advanced_view, on_press: Some(tab1) },
///     ],
/// }
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tabs {
    pub active_index: usize,
    pub items: Vec<TabItem>,
}

impl<S: fission_core::AppState> Widget<S> for Tabs {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.tabs;
        let mut tab_buttons = vec![];

        for (i, item) in self.items.iter().enumerate() {
            let is_active = i == self.active_index;
            let color = if is_active {
                theme.active_color
            } else {
                theme.inactive_color
            };

            let tab_button = VStack {
                spacing: Some(0.0),
                children: vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(
                            Text::new(item.title.clone())
                                .size(14.0)
                                .color(color)
                                .into_node(),
                        )),
                        on_press: item.on_press.clone(),
                        height: Some(38.0),
                        padding: Some([10.0, 10.0, 0.0, 0.0]),
                        ..Default::default()
                    }
                    .into_node(),
                    if is_active {
                        Container::new(
                            fission_core::ui::widgets::spacer::Spacer::default().into_node(),
                        )
                        .height(theme.indicator_height)
                        .bg(theme.active_color)
                        .into_node()
                    } else {
                        fission_core::ui::widgets::spacer::Spacer::default().into_node()
                    },
                ],
            }
            .into_node();

            tab_buttons.push(Container::new(tab_button).padding_all(2.0).into_node());
        }

        let tab_bar = Container::new(
            HStack {
                spacing: Some(14.0),
                children: tab_buttons,
            }
            .into_node(),
        )
        .bg(theme.background)
        .border(theme.divider_color, 1.0)
        .padding_all(2.0)
        .into_node();

        VStack {
            spacing: Some(12.0),
            children: vec![
                tab_bar,
                if let Some(tab) = self.items.get(self.active_index) {
                    tab.content.clone()
                } else {
                    fission_core::ui::widgets::spacer::Spacer::default().into_node()
                },
            ],
        }
        .into_node()
    }
}
