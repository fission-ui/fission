use crate::canvas_panel::CanvasPanel;
use crate::viewer_panel::ViewerPanel;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct WideExampleLayout {
    pub edit_canvas: ActionEnvelope,
    pub viewer_camera: ActionEnvelope,
    pub canvas_camera: ActionEnvelope,
}

impl From<WideExampleLayout> for Widget {
    fn from(layout: WideExampleLayout) -> Self {
        let (_, view) = fission::build::current::<crate::state::CanvasExampleState>();
        Row {
            id: Some(WidgetId::explicit("interactive-canvas.layout.wide")),
            gap: Some(view.env().theme.tokens.spacing.l),
            children: widgets![
                ViewerPanel {
                    instance: "wide",
                    height: 520.0,
                    camera_action: layout.viewer_camera,
                },
                CanvasPanel {
                    instance: "wide",
                    height: 520.0,
                    edit_action: layout.edit_canvas,
                    camera_action: layout.canvas_camera,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(crate) struct CompactExampleLayout {
    pub edit_canvas: ActionEnvelope,
    pub viewer_camera: ActionEnvelope,
    pub canvas_camera: ActionEnvelope,
}

impl From<CompactExampleLayout> for Widget {
    fn from(layout: CompactExampleLayout) -> Self {
        let (_, view) = fission::build::current::<crate::state::CanvasExampleState>();
        Column {
            id: Some(WidgetId::explicit("interactive-canvas.layout.compact")),
            gap: Some(view.env().theme.tokens.spacing.l),
            children: widgets![
                ViewerPanel {
                    instance: "compact",
                    height: 360.0,
                    camera_action: layout.viewer_camera,
                },
                CanvasPanel {
                    instance: "compact",
                    height: 420.0,
                    edit_action: layout.edit_canvas,
                    camera_action: layout.canvas_camera,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
