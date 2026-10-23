use crate::layout::COLOR_DOT_SIZE;
use fission::prelude::*;

fission_component! {
    pub(super) ColorDot<crate::state::AnimationGalleryState> { color: Color }
    |dot, _state, _cx| {} =>
        Container(Text(" "))
            .width(COLOR_DOT_SIZE)
            .height(COLOR_DOT_SIZE)
            .border_radius(COLOR_DOT_SIZE / 2.0)
            .bg(dot.color)
}
