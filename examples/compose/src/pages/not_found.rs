use crate::actions::{navigate, Navigate};
use crate::state::AppState;
use fission::prelude::*;

Component! {
    pub NotFound<AppState> |page, state, cx| =>
    Column(
        gap: 20.0,
        children: [
            Text::new(cx.tr("not_found.title")).size(28.0),
            Text(cx.tr("not_found.message").replace("{path}", &state.current_path)),
            Row(children: [
                Button(
                    child: Text(cx.tr("not_found.return")),
                    key: "dashboard",
                    on_press: cx.bind(Navigate("/".into()), reduce!(navigate)),
                ),
            ]),
        ],
    )
}
