use crate::state::{
    on_set_design_system, on_toggle_design_system_menu, DesignSystemChoice, SetDesignSystem,
    ShowcaseState, ToggleDesignSystemMenu,
};
use fission::prelude::*;
use fission::widgets::{Select, SelectItem};

/// Wide enough for the longest look's name, since the menu matches its trigger.
const PICKER_WIDTH: f32 = 160.0;

/// Chooses the design system the preview renders with.
///
/// Examples read component recipes instead of hard-coding their styling, so one
/// choice here re-skins whichever example is open. A select keeps seven looks to
/// one compact trigger.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DesignSystemPicker;

impl From<DesignSystemPicker> for Widget {
    fn from(_component: DesignSystemPicker) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let state = view.state();
        let items = DesignSystemChoice::ALL
            .iter()
            .map(|choice| SelectItem {
                label: view.tr(choice.label_key()),
                icon: None,
                on_select: with_reducer!(ctx, SetDesignSystem(*choice), on_set_design_system),
                semantics_identifier: Some(format!(
                    "showcase.preview.design_system.{}",
                    choice.slug()
                )),
            })
            .collect();

        SemanticsRegion::new(Select {
            id: WidgetId::explicit("showcase.preview.design_system.select"),
            selected_label: Some(view.tr(state.design_system.label_key())),
            items,
            is_open: state.design_system_open,
            on_toggle: Some(with_reducer!(
                ctx,
                ToggleDesignSystemMenu,
                on_toggle_design_system_menu
            )),
            trigger_semantics_identifier: Some("showcase.preview.design_system".into()),
            placeholder: view.tr("showcase.workbench.design_system"),
            width: Some(PICKER_WIDTH),
        })
        .role(Role::Group)
        .label(view.tr("showcase.workbench.design_system"))
        .into()
    }
}
