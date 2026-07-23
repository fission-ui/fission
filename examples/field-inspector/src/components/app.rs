use crate::components::app_compact::FieldInspectorCompact;
use crate::components::app_expanded::FieldInspectorExpanded;
use crate::model::FieldInspectorState;
use fission::prelude::*;

const EXPANDED_BREAKPOINT: f32 = 1_100.0;

#[derive(Clone)]
pub struct FieldInspectorApp;

impl From<FieldInspectorApp> for Widget {
    fn from(_: FieldInspectorApp) -> Self {
        let (_ctx, _view) = fission::build::current::<FieldInspectorState>();

        Responsive::new(FieldInspectorCompact)
            .id(WidgetId::explicit("field-inspector.responsive"))
            .case(ResponsiveCase::min_width(
                EXPANDED_BREAKPOINT,
                FieldInspectorExpanded,
            ))
            .into()
    }
}
