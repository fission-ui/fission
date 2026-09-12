use crate::stack::HStack;
use fission_core::op::CornerRadii;
use fission_core::ui::{Button, ButtonVariant, Container, SemanticsRegion, Text, Widget};
use fission_core::ActionEnvelope;
use fission_ir::{Role, SemanticOrientation};
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
    /// Segment labels in display order.
    pub options: Vec<String>,
    /// Zero-based controlled selected segment.
    pub selected_index: usize,
    /// Factory producing an action for a requested selection index.
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

impl From<SegmentedControl> for Widget {
    fn from(component: SegmentedControl) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.segmented_control;
        let tokens = &view.env().theme.tokens;
        let mut children = Vec::new();

        for (i, opt) in this.options.iter().enumerate() {
            let is_selected = i == this.selected_index;
            let cb = this.on_change.clone();

            let button: Widget = SemanticsRegion::new(Button {
                variant: if is_selected {
                    ButtonVariant::Filled
                } else {
                    ButtonVariant::Ghost
                },
                child: Some(
                    Text::new(opt.clone())
                        .size(tokens.typography.body_medium_size)
                        .color(if is_selected {
                            theme.active_text
                        } else {
                            tokens.colors.text_primary
                        })
                        .into(),
                ),
                padding: Some([tokens.spacing.s, tokens.spacing.s, 0.0, 0.0]),
                on_press: cb.map(|f| f(i)),
                ..Default::default()
            })
            // One-of-many selection, so each segment is a radio rather than an
            // unrelated button. This also puts the group in the composite
            // keyboard contract, giving it arrow navigation over its segments.
            .role(Role::Radio)
            .label(opt.clone())
            .selected(is_selected)
            .into();

            // End caps follow the track's own radius, so the first and last
            // segments sit flush inside it instead of being square against a
            // rounded track. Until the IR carried per-corner radii this was
            // simply not expressible, which is why every segment looked the
            // same regardless of position.
            let inner_radius = (theme.radius - 1.0).max(0.0);
            let segment_radii = match (i == 0, i + 1 == this.options.len()) {
                (true, true) => CornerRadii::uniform(inner_radius),
                (true, false) => CornerRadii::left(inner_radius),
                (false, true) => CornerRadii::right(inner_radius),
                (false, false) => CornerRadii::uniform(0.0),
            };
            children.push(
                Container::new(button)
                    .flex_grow(1.0)
                    .border_radii(segment_radii)
                    .into(),
            );
        }

        SemanticsRegion::new(
            Container::new(HStack {
                spacing: Some(2.0),
                children,
            })
            .padding_all(1.0)
            .bg(theme.bg_color)
            .border(theme.border_color, 1.0)
            .border_radius(theme.radius),
        )
        .role(Role::RadioGroup)
        .orientation(SemanticOrientation::Horizontal)
        .into()
    }
}
