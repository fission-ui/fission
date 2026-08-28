use fission_core::ui::{Align, Widget};
use serde::{Deserialize, Serialize};

/// Centers its child both horizontally and vertically within the available space.
///
/// A convenience wrapper around [`Align`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Center {
    /// Content positioned at the center of the available horizontal and
    /// vertical space.
    pub child: Widget,
}

impl From<Center> for Widget {
    fn from(component: Center) -> Self {
        let this = &component;

        Align::new(this.child.clone()).into()
    }
}
