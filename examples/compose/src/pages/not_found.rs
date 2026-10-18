use crate::actions::{navigate, Navigate};
use crate::state::AppState;
use fission::prelude::*;

Component! {
    pub NotFound<AppState> |page, state, cx| =>
    Page(
        cx.tr("not_found.title"),
        content: Text(cx.tr("not_found.message").replace("{path}", &state.current_path)),
        actions: [
            Button(
                child: Text(cx.tr("not_found.return")),
                key: "dashboard",
                on_press: cx.bind(Navigate("/".into()), reduce!(navigate)),
            ),
        ],
    )
}
