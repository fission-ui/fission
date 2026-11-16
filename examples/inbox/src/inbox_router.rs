use crate::components::{EmailDetail, EmailList};
use crate::model::InboxState;
use fission::prelude::*;

fission_component! {
    pub(crate) InboxRouter<InboxState> |_router, state, _cx| =>
    Router::<InboxState>::new()
        .with_path(state.current_path.clone())
        .route_component("/inbox", EmailList::new("Inbox".into()))
        .route_component("/:folder/:id", EmailDetailRoute)
        .route_component("/:folder", EmailListRoute)
        .not_found(|| Text::new("Folder not found"))
}

fission_component! {
    EmailDetailRoute<InboxState> |_route, _state, _cx| {
        let params = fission::build::read::<RouteParams>();
    } =>
    EmailDetail(
        params["folder"].clone(),
        params["id"].parse().unwrap_or(0),
    )
}

fission_component! {
    EmailListRoute<InboxState> |_route, _state, _cx| {
        let params = fission::build::read::<RouteParams>();
    } =>
    EmailList(params["folder"].clone())
}
