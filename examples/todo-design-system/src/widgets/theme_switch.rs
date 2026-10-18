use crate::state::{set_theme_mode, SetThemeMode, TodoState};
use fission::prelude::*;
use std::sync::Arc;

const MODES: [(DesignMode, &str); 2] = [
    (DesignMode::Light, "todo.theme.light"),
    (DesignMode::Dark, "todo.theme.dark"),
];

fission_component! {
/// Switches the generated theme between its light and dark modes.
    pub(super) ThemeSwitch<TodoState> |_switch, state, cx| {
        let actions: Vec<ActionEnvelope> = MODES
            .iter()
            .map(|(mode, _)| cx.bind(SetThemeMode(*mode), reduce_with!(set_theme_mode)))
            .collect();
        let selected_index = MODES
            .iter()
            .position(|(mode, _)| *mode == state.theme_mode)
            .unwrap_or(0);
    } =>
        SemanticsRegion(SegmentedControl(
            options: MODES
                .iter()
                .map(|(_, key)| cx.view.tr(key))
                .collect::<Vec<_>>(),
            selected_index: selected_index,
            on_change: Arc::new(move |index| actions[index].clone()),
        ))
        .role(Role::Group)
        .label(cx.view.tr("todo.theme.label"))
        .identifier("todo.theme")
}
