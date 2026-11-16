use fission::prelude::*;

fission_component! {
    pub NotFoundPage<crate::state::AnimationGalleryState> |_page, _state, _cx| {} =>
        Text("404: animation gallery page not found")
}
