use crate::state::AppState;
use fission::prelude::*;

fission_component! {
    #[derive(Default)]
    pub StatusPill<AppState> {
        props {
            text: String,
            active: bool,
        }
    }

    view(pill, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(Text(pill.text.clone()))
        .padding([
            tokens.spacing.xs,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.s,
        ])
        .border_radius(tokens.radii.full)
        .bg(if pill.active {
            tokens.colors.success
        } else {
            tokens.colors.surface_sunken
        })
}
