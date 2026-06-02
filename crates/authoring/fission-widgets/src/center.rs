use fission_core::ui::{Align, Widget};
use serde::{Deserialize, Serialize};

/// Centers its child both horizontally and vertically within the available space.
///
/// A convenience wrapper around [`Align`](fission_core::ui::Align).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Center {
    pub child: Widget,
}

impl From<Center> for Widget {
    fn from(component: Center) -> Self {
        let this = &component;

        Align::new(this.child.clone()).into()
    }
}
