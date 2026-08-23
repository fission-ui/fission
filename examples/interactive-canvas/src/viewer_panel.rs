use crate::viewer_scene::ViewerScene;
use fission::prelude::*;

pub(crate) struct ViewerPanel {
    pub instance: &'static str,
    pub height: f32,
    pub camera_action: ActionEnvelope,
}

impl From<ViewerPanel> for Widget {
    fn from(panel: ViewerPanel) -> Self {
        let (_, view) = fission::build::current::<crate::state::CanvasExampleState>();
        let tokens = &view.env().theme.tokens;
        let viewer_id = WidgetId::explicit(&format!("interactive-viewer.{}", panel.instance));
        let viewer: Widget = InteractiveViewer {
            id: Some(viewer_id),
            child: ViewerScene {
                instance: panel.instance,
            }
            .into(),
            initial_transform: ViewportTransform::new(12.0, 12.0, 0.82),
            min_scale: 0.35,
            max_scale: 4.0,
            on_interaction_start: Some(panel.camera_action.clone()),
            on_interaction_update: Some(panel.camera_action.clone()),
            on_interaction_end: Some(panel.camera_action),
            ..Default::default()
        }
        .into();

        Column {
            flex_grow: 1.0,
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new("InteractiveViewer")
                    .size(tokens.typography.heading2_size)
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text::new("A camera around any retained widget subtree")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
                Container::new(viewer)
                    .height(panel.height)
                    .flex_grow(1.0)
                    .bg(tokens.colors.surface)
                    .border(tokens.colors.border, 1.0)
                    .border_radius(20.0),
            ],
            ..Default::default()
        }
        .into()
    }
}
