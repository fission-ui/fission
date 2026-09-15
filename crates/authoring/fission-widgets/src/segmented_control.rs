use crate::stack::HStack;
use fission_core::op::Fill;
use fission_core::ui::{
    Button, ButtonStyleOverride, ButtonVariant, Container, SemanticsRegion, Text, Widget,
};
use fission_core::ActionEnvelope;
use fission_ir::{Role, SemanticOrientation, Semantics};
use std::sync::Arc;

/// A horizontal row of toggle buttons where exactly one option is active.
///
/// The selected segment is a raised surface pill; the others are quiet text. The segments sit inside a padded,
/// bordered track, and their corners are concentric with the track's.
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
        let track_inset = tokens.spacing.xs;
        let segment_radius = fission_theme::concentric_radius(theme.radius, track_inset);
        let mut children = Vec::new();

        for (i, opt) in this.options.iter().enumerate() {
            let is_selected = i == this.selected_index;
            let cb = this.on_change.clone();

            // One-of-many selection, so each segment is a radio rather than an unrelated button.
            // This also puts the group in the composite keyboard contract, giving it arrow
            // navigation over its segments. The button carries these semantics itself; a wrapping
            // region would add a generic node around it.
            let button: Widget = Button {
                semantics: Some(Semantics {
                    role: Role::Radio,
                    label: Some(opt.clone()),
                    selected: Some(is_selected),
                    focusable: true,
                    ..Semantics::default()
                }),
                variant: ButtonVariant::Ghost,
                child: Some(
                    Text::new(opt.clone())
                        .size(tokens.typography.body_medium_size)
                        .color(if is_selected {
                            tokens.colors.text_primary
                        } else {
                            tokens.colors.text_secondary
                        })
                        .into(),
                ),
                padding: Some([tokens.spacing.ms, tokens.spacing.ms, 0.0, 0.0]),
                // Segments sit inside the padded track, so their corners are the
                // track's corners less the inset. Mismatched curves on nested shapes
                // read as misaligned even when nothing else is.
                // The selected segment is a raised surface on the sunken track, the same
                // treatment as a selected tab, so both one-of-many controls read alike.
                style: Some(ButtonStyleOverride {
                    corner_radius: Some(segment_radius),
                    background_fill: is_selected.then(|| Fill::Solid(tokens.colors.surface_raised)),
                    shadows: is_selected.then(|| tokens.elevations.level1.into_iter().collect()),
                    ..Default::default()
                }),
                on_press: cb.map(|f| f(i)),
                ..Default::default()
            }
            .into();

            children.push(Container::new(button).flex_shrink(0.0).into());
        }

        let track: Widget = SemanticsRegion::new(
            Container::new(HStack {
                spacing: Some(track_inset / 2.0),
                children,
            })
            .padding_all(track_inset)
            // The bordered track must not shrink below its segments, or a crowded row squeezes
            // the border box and clips its trailing edge.
            .flex_shrink(0.0)
            .bg(tokens.colors.surface_sunken)
            .border(theme.border_color, 1.0)
            .border_radius(theme.radius),
        )
        .role(Role::RadioGroup)
        .orientation(SemanticOrientation::Horizontal)
        .into();

        // The track hugs its segments. Stretched to whatever width a parent offers, a
        // three-option control became a bar across the page and each option a far-apart
        // target (Fitts's law, law of proximity). A row lays the track out at its natural
        // width even when the row itself is stretched.
        HStack {
            spacing: None,
            children: vec![track],
        }
        .into()
    }
}
