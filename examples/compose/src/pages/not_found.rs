use crate::actions::{navigate, Navigate};
use crate::state::AppState;
use fission::prelude::*;

fission_component! {
    pub NotFound<AppState> |page, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(
        gap: tokens.spacing.ml,
        children: [
            Text(cx.tr("not_found.title")).size(tokens.typography.heading1_size),
            Text(cx.tr("not_found.message").replace("{path}", &state.current_path)),
            Row(gap: tokens.spacing.s, children: [
                Button(
                    child: Text(cx.tr("not_found.return")),
                    key: "dashboard",
                    on_press: cx.bind(Navigate("/".into()), reduce!(navigate)),
                ),
            ]),
        ],
    )
}
