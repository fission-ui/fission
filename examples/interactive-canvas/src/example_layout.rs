use crate::canvas_panel::CanvasPanel;
use crate::state::CanvasExampleState;
use crate::viewer_panel::ViewerPanel;
use fission::prelude::*;

fission_component! {
    pub(crate) WideExampleLayout<CanvasExampleState> {
        edit_canvas: ActionEnvelope,
        viewer_camera: ActionEnvelope,
        canvas_camera: ActionEnvelope,
    } |layout, _state, cx| {} =>
        Row(
            id: WidgetId::explicit("interactive-canvas.layout.wide"),
            gap: cx.view.env().theme.tokens.spacing.l,
            children: [
                ViewerPanel("wide", 520.0, layout.viewer_camera),
                CanvasPanel(
                    "wide",
                    520.0,
                    layout.edit_canvas,
                    layout.canvas_camera,
                ),
            ],
        )
}

fission_component! {
    pub(crate) CompactExampleLayout<CanvasExampleState> {
        edit_canvas: ActionEnvelope,
        viewer_camera: ActionEnvelope,
        canvas_camera: ActionEnvelope,
    } |layout, _state, cx| {} =>
        Column(
            id: WidgetId::explicit("interactive-canvas.layout.compact"),
            gap: cx.view.env().theme.tokens.spacing.l,
            children: [
                ViewerPanel("compact", 360.0, layout.viewer_camera),
                CanvasPanel(
                    "compact",
                    420.0,
                    layout.edit_canvas,
                    layout.canvas_camera,
                ),
            ],
        )
}
