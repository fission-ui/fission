use crate::state::{
    on_set_design_system, on_toggle_design_system_menu, DesignSystemChoice, SetDesignSystem,
    ShowcaseState, ToggleDesignSystemMenu,
};
use fission::prelude::*;
use fission::widgets::{Select, SelectItem};

/// Wide enough for the longest look's name, since the menu matches its trigger.
const PICKER_WIDTH: f32 = 160.0;

fission_component! {
/// Chooses the design system the preview renders with.
///
/// Examples read component recipes instead of hard-coding their styling, so one
/// choice here re-skins whichever example is open. A select keeps seven looks to
/// one compact trigger.
    #[derive(Clone, Copy, Debug)] pub(crate) DesignSystemPicker<ShowcaseState> {}
    |_component, state, cx| {
        let items: Vec<SelectItem> = DesignSystemChoice::ALL
            .iter()
            .map(|choice| SelectItem {
                label: cx.view.tr(choice.label_key()),
                icon: None,
                on_select: with_reducer!(cx.build, SetDesignSystem(*choice), on_set_design_system),
                semantics_identifier: Some(format!(
                    "showcase.preview.design_system.{}",
                    choice.slug()
                )),
                form_value: None,
            })
            .collect();

    } =>
        SemanticsRegion(Select(
            id: WidgetId::explicit("showcase.preview.design_system.select"),
            selected_label: Some(cx.view.tr(state.design_system.label_key())),
            items: items,
            is_open: state.design_system_open,
            on_toggle: with_reducer!(
                cx.build,
                ToggleDesignSystemMenu,
                on_toggle_design_system_menu
            ),
            trigger_semantics_identifier: Some("showcase.preview.design_system".into()),
            placeholder: cx.view.tr("showcase.workbench.design_system"),
            width: Some(PICKER_WIDTH),
        ))
        .role(Role::Group)
        .label(cx.view.tr("showcase.workbench.design_system"))
}
