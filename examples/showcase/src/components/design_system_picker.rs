use crate::state::{on_set_design_system, DesignSystemChoice, SetDesignSystem, ShowcaseState};
use fission::prelude::*;
use std::sync::Arc;

/// Switches every preview between the supplied design systems.
///
/// Examples render with the showcase's theme, so one choice here re-skins all of
/// them. That's the point of reading component recipes instead of hard-coding
/// styling in each widget.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DesignSystemPicker;

impl From<DesignSystemPicker> for Widget {
    fn from(_component: DesignSystemPicker) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let actions: Vec<ActionEnvelope> = DesignSystemChoice::ALL
            .iter()
            .map(|choice| with_reducer!(ctx, SetDesignSystem(*choice), on_set_design_system))
            .collect();
        let selected_index = DesignSystemChoice::ALL
            .iter()
            .position(|choice| *choice == view.state().design_system)
            .unwrap_or(0);

        SemanticsRegion::new(SegmentedControl {
            options: DesignSystemChoice::ALL
                .iter()
                .map(|choice| view.tr(choice.label_key()))
                .collect(),
            selected_index,
            on_change: Some(Arc::new(move |index| actions[index].clone())),
        })
        .role(Role::Group)
        .label(view.tr("showcase.workbench.design_system"))
        .identifier("showcase.preview.design_system")
        .into()
    }
}
