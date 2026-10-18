use crate::state::CanvasExampleState;
use crate::viewer_scene::ViewerScene;
use fission::prelude::*;

fission_component! {
    pub(crate) ViewerPanel<CanvasExampleState> {
        instance: &'static str,
        height: f32,
        camera_action: ActionEnvelope,
    } |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            flex_grow: 1.0,
            gap: tokens.spacing.s,
            children: [
                Text("InteractiveViewer")
                    .size(tokens.typography.heading2_size)
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text("A camera around any retained widget subtree")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
                Container(
                    InteractiveViewer(
                        id: WidgetId::explicit(&format!(
                            "interactive-viewer.{}",
                            panel.instance,
                        )),
                        child: ViewerScene(panel.instance),
                        initial_transform: ViewportTransform::new(12.0, 12.0, 0.82),
                        min_scale: 0.35,
                        max_scale: 4.0,
                        on_interaction_start: panel.camera_action.clone(),
                        on_interaction_update: panel.camera_action.clone(),
                        on_interaction_end: panel.camera_action,
                    ),
                )
                .height(panel.height)
                .flex_grow(1.0)
                .bg(tokens.colors.surface)
                .border(tokens.colors.border, 1.0)
                .border_radius(20.0),
            ],
        )
}
