use crate::stack::{HStack, VStack};
use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{Column, Container, Node, Row};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplitView {
    pub id: WidgetNodeId,
    pub direction: SplitDirection,
    pub first: Box<Node>,
    pub second: Box<Node>,
    pub split_ratio: f32,                  // 0.0 to 1.0
    pub on_resize: Option<ActionEnvelope>, // Action(f32)
}

// Internal action to track dragging
#[derive(Clone, Debug, Serialize, Deserialize, fission_macros::Action)]
struct DragHandle {
    delta: f32,
}

// Since we can't easily capture local drag state without a reducer in the user app,
// SplitView relies on the user providing `split_ratio` and `on_resize`.
// However, to make it drag, we need a Handle widget that emits drag events.
// fission-core Input system supports `PointerEvent::Move` when pressed.
// But we need to translate that into a ratio change.
// The ratio change depends on the size of the container.
// This logic is complex for a purely declarative widget without layout readback.
//
// Workaround: We use a "Drag" action that passes pixel delta.
// The user's reducer updates the ratio based on assumed width/height.
// Better: The `on_resize` action payload could be the *new ratio* if the engine computed it?
// Engine doesn't compute high level logic.
//
// For MVP: We will simply assume the handle reports a Delta.
// The user must handle normalization.
// Or we provide a helper Action?
//
// Let's implement the Visuals first. `flex_grow` is perfect for this.
// First pane: flex_grow = ratio.
// Second pane: flex_grow = 1.0 - ratio.

impl<S: fission_core::AppState> Widget<S> for SplitView {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let handle_size = 4.0;
        let tokens = &view.env.theme.tokens;

        let (width, height) = match self.direction {
            SplitDirection::Horizontal => (Some(handle_size), None),
            SplitDirection::Vertical => (None, Some(handle_size)),
        };

        let (line_w, line_h) = match self.direction {
            SplitDirection::Horizontal => (Some(1.0), None),
            SplitDirection::Vertical => (None, Some(1.0)),
        };

        let line = Container::new(fission_core::ui::widgets::Spacer::default().into_node())
            .bg(tokens.colors.border)
            .into_node();

        // We need the line to be 1px. Spacer doesn't support background color directly (it's empty).
        // Container supports background.
        // So we use Container(Spacer) for line.
        // And we set width/height on that Container.

        let mut line_container =
            Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                .bg(tokens.colors.border)
                .flex_grow(1.0);

        if let Some(w) = line_w {
            line_container = line_container.width(w);
        }
        if let Some(h) = line_h {
            line_container = line_container.height(h);
        }

        let mut handle = Container::new(line_container.into_node()).bg(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }) // Transparent hit area
        .flex_shrink(0.0);

        if let Some(w) = width {
            handle = handle.width(w);
        }
        if let Some(h) = height {
            handle = handle.height(h);
        }

        // Ensure ratio is clamped
        let ratio = self.split_ratio.clamp(0.1, 0.9);
        let first_grow = ratio;
        let second_grow = 1.0 - ratio;

        // Wrap children in Containers with flex_grow
        let first_pane = Container::new(*self.first.clone())
            .flex_grow(first_grow)
            .flex_shrink(1.0)
            .into_node();

        let second_pane = Container::new(*self.second.clone())
            .flex_grow(second_grow)
            .flex_shrink(1.0)
            .into_node();

        match self.direction {
            SplitDirection::Horizontal => Row {
                children: vec![first_pane, handle.into_node(), second_pane],
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: fission_ir::op::JustifyContent::Start,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }
            .into_node(),
            SplitDirection::Vertical => Column {
                children: vec![first_pane, handle.into_node(), second_pane],
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: fission_ir::op::JustifyContent::Start,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }
            .into_node(),
        }
    }
}
