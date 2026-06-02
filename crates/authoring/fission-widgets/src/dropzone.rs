use fission_core::ui::widgets::GestureDetector;
use fission_core::ui::Widget;
use fission_core::ActionEnvelope;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dropzone {
    pub child: Widget,
    pub on_drop: Option<ActionEnvelope>,
    pub on_drag_enter: Option<ActionEnvelope>,
    pub on_drag_leave: Option<ActionEnvelope>,
}

impl From<Dropzone> for Widget {
    fn from(component: Dropzone) -> Self {
        let this = &component;

        GestureDetector {
            child: this.child.clone(),
            on_drop: this.on_drop.clone(),
            on_drag_enter: this.on_drag_enter.clone(),
            on_drag_leave: this.on_drag_leave.clone(),
            ..Default::default()
        }
        .into()
    }
}
