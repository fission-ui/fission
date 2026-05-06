use crate::stack::HStack;
use fission_core::ui::{Button, ButtonVariant, Container, Node, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use std::sync::Arc;

/// A horizontal row of toggle buttons where exactly one option is active.
///
/// The active segment uses `ButtonVariant::Filled` with the theme's active color.
/// Inactive segments use `ButtonVariant::Ghost`. The entire control is wrapped in
/// a bordered, rounded container.
///
/// # Fields
///
/// * `options` - The label text for each segment.
/// * `selected_index` - Index of the currently active segment.
/// * `on_change` - Closure that produces an action for the newly selected index.
pub struct SegmentedControl {
    pub options: Vec<String>,
    pub selected_index: usize,
    pub on_change: Option<Arc<dyn Fn(usize) -> ActionEnvelope + Send + Sync>>,
}

// Manual Debug
impl std::fmt::Debug for SegmentedControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentedControl")
            .field("options", &self.options)
            .field("selected", &self.selected_index)
            .finish()
    }
}

impl<S: fission_core::AppState> Widget<S> for SegmentedControl {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.segmented_control;
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();

        for (i, opt) in self.options.iter().enumerate() {
            let is_selected = i == self.selected_index;
            let cb = self.on_change.clone();

            let button = Button {
                variant: if is_selected {
                    ButtonVariant::Filled
                } else {
                    ButtonVariant::Ghost
                },
                child: Some(Box::new(
                    Text::new(opt.clone())
                        .size(14.0)
                        .color(if is_selected {
                            theme.active_text
                        } else {
                            tokens.colors.text_primary
                        })
                        .into_node(),
                )),
                height: Some(40.0),
                padding: Some([12.0, 12.0, 0.0, 0.0]),
                on_press: cb.map(|f| f(i)),
                ..Default::default()
            }
            .into_node();

            children.push(Container::new(button).flex_grow(1.0).into_node());
        }

        Container::new(
            HStack {
                spacing: Some(2.0),
                children,
            }
            .into_node(),
        )
        .padding_all(1.0)
        .bg(theme.bg_color)
        .border(theme.border_color, 1.0)
        .border_radius(theme.radius)
        .into_node()
    }
}
