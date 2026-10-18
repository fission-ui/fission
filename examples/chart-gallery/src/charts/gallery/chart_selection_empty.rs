use fission::prelude::*;

use crate::state::GalleryState;

fission_component! {
    pub(super) ChartSelectionEmpty<GalleryState> {}
    |_empty, _state, _cx| {} =>
        Container(Text("Select a chart from the gallery").color(Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        }))
}
