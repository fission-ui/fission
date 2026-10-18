use crate::actions::{navigate, Navigate};
use crate::state::AppState;
use fission::prelude::*;

fission_component! {
    pub Navigation<AppState> |navigation, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Row(gap: tokens.spacing.s, children: [
        Button(
            child: Text(cx.tr("nav.dashboard")),
            key: "dashboard",
            variant: if state.current_path == "/" { ButtonVariant::Primary } else { ButtonVariant::Ghost },
            on_press: cx.bind(Navigate("/".into()), reduce!(navigate)),
        ),
        Button(
            child: Text(cx.tr("nav.products")),
            key: "products",
            variant: if state.current_path.starts_with("/products") { ButtonVariant::Primary } else { ButtonVariant::Ghost },
            on_press: cx.bind(Navigate("/products".into()), reduce!(navigate)),
        ),
        Button(
            child: Text(cx.tr("nav.settings")),
            key: "settings",
            variant: if state.current_path == "/settings" { ButtonVariant::Primary } else { ButtonVariant::Ghost },
            on_press: cx.bind(Navigate("/settings".into()), reduce!(navigate)),
        ),
    ])
}
