use crate::stack::{HStack, VStack};
use fission_core::ui::{
    Button, ButtonVariant, ComponentSize, ComponentState, Container, Node, Text,
};
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
    pub size: ComponentSize,
}

impl<S: fission_core::AppState> Widget<S> for Tabs {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let theme = &view.env.theme.components.tabs;
            let mut tab_buttons = vec![];

            for (i, item) in self.items.iter().enumerate() {
                let is_active = i == self.active_index;
                let state = if is_active {
                    ComponentState::Active
                } else {
                    ComponentState::Default
                };
                let style = theme.resolve_tab(self.size, state);
                let color = style.text_color.unwrap_or(if is_active {
                    theme.active_color
                } else {
                    theme.inactive_color
                });
                let border = style.border.clone();

                let tab_button = VStack {
                    spacing: Some(0.0),
                    children: vec![
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Box::new(
                                Text::new(item.title.clone())
                                    .size(style.font_size.unwrap_or(14.0))
                                    .weight(style.font_weight.unwrap_or(400))
                                    .color(color)
                                    .into_node(),
                            )),
                            on_press: item.on_press.clone(),
                            height: style.height.or(Some(38.0)),
                            padding: Some([
                                10.0,
                                10.0,
                                style.padding_y.unwrap_or(0.0),
                                style.padding_y.unwrap_or(0.0),
                            ]),
                            ..Default::default()
                        }
                        .into_node(),
                        if is_active {
                            Container::new(
                                fission_core::ui::widgets::spacer::Spacer::default().into_node(),
                            )
                            .height(
                                border
                                    .as_ref()
                                    .map(|border| border.width)
                                    .unwrap_or(theme.indicator_height),
                            )
                            .bg(match border.map(|border| border.fill) {
                                Some(fission_core::op::Fill::Solid(color)) => color,
                                _ => theme.active_color,
                            })
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
            .bg_fill(
                theme
                    .track_style
                    .background
                    .clone()
                    .unwrap_or(fission_core::op::Fill::Solid(theme.background)),
            )
            .border(
                theme
                    .track_style
                    .border
                    .as_ref()
                    .and_then(|border| match &border.fill {
                        fission_core::op::Fill::Solid(color) => Some(*color),
                        _ => None,
                    })
                    .unwrap_or(theme.divider_color),
                theme
                    .track_style
                    .border
                    .as_ref()
                    .map(|border| border.width)
                    .unwrap_or(1.0),
            )
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
        })
    }
}
