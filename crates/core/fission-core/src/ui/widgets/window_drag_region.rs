use crate::ui::{SemanticsRegion, Widget};
use fission_ir::WidgetId;
use serde::{Deserialize, Serialize};

/// Marks the draggable part of a custom desktop title bar.
///
/// The desktop shell starts the OS move operation on the native primary-button
/// press. Interactive descendants, such as buttons and text inputs, take
/// precedence. Double-clicking the drag surface toggles maximization. Other
/// shells render the child normally without a window move operation.
///
/// Use with `DesktopApp::with_decorations(false)`. This widget does not style
/// the title bar or add window buttons; compose those from ordinary widgets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowDragRegion {
    pub id: Option<WidgetId>,
    pub semantics_identifier: Option<String>,
    pub child: Widget,
}

impl WindowDragRegion {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            id: None,
            semantics_identifier: None,
            child: child.into(),
        }
    }

    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<WindowDragRegion> for Widget {
    fn from(region: WindowDragRegion) -> Self {
        SemanticsRegion {
            id: region.id,
            identifier: region.semantics_identifier,
            window_drag_region: true,
            focusable: Some(false),
            child: Some(region.child),
            ..Default::default()
        }
        .into()
    }
}
