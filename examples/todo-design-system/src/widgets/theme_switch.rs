use crate::state::{set_theme_mode, SetThemeMode, TodoState};
use fission::prelude::*;
use std::sync::Arc;

const MODES: [(DesignMode, &str); 2] = [
    (DesignMode::Light, "todo.theme.light"),
    (DesignMode::Dark, "todo.theme.dark"),
];

/// Switches the generated theme between its light and dark modes.
pub(super) struct ThemeSwitch;

impl From<ThemeSwitch> for Widget {
    fn from(_switch: ThemeSwitch) -> Self {
        let (ctx, view) = fission::build::current::<TodoState>();
        let actions: Vec<ActionEnvelope> = MODES
            .iter()
            .map(|(mode, _)| ctx.bind(SetThemeMode(*mode), reduce_with!(set_theme_mode)))
            .collect();
        let selected_index = MODES
            .iter()
            .position(|(mode, _)| *mode == view.state().theme_mode)
            .unwrap_or(0);

        SemanticsRegion::new(SegmentedControl {
            options: MODES.iter().map(|(_, key)| view.tr(key)).collect(),
            selected_index,
            on_change: Some(Arc::new(move |index| actions[index].clone())),
        })
        .role(Role::Group)
        .label(view.tr("todo.theme.label"))
        .identifier("todo.theme")
        .into()
    }
}
