use fission::prelude::*;

pub(crate) struct ViewerScene {
    pub instance: &'static str,
}

impl From<ViewerScene> for Widget {
    fn from(scene: ViewerScene) -> Self {
        let (_, view) = fission::build::current::<crate::state::CanvasExampleState>();
        let tokens = &view.env().theme.tokens;
        let tile = |id: &str, x, y, width, height, label: &str, primary: bool| Positioned {
            id: Some(WidgetId::explicit(&format!(
                "interactive-viewer.{}.tile.{id}",
                scene.instance
            ))),
            left: Some(x),
            top: Some(y),
            width: Some(width),
            height: Some(height),
            child: Some(
                Container::new(Align::new(Text::new(label).weight(700).color(if primary {
                    tokens.colors.on_primary
                } else {
                    tokens.colors.text_primary
                })))
                .bg(if primary {
                    tokens.colors.primary
                } else {
                    tokens.colors.surface_raised
                })
                .border(tokens.colors.border, 1.0)
                .border_radius(18.0)
                .into(),
            ),
            ..Default::default()
        };

        Container::new(ZStack {
            id: Some(WidgetId::explicit(&format!(
                "interactive-viewer.{}.scene",
                scene.instance
            ))),
            children: widgets![
                tile("origin", 80.0, 80.0, 170.0, 110.0, "Pan anywhere", true),
                tile("zoom", 330.0, 160.0, 190.0, 120.0, "Pinch or zoom", false),
                tile(
                    "retained",
                    160.0,
                    350.0,
                    220.0,
                    100.0,
                    "Retained widgets",
                    false
                ),
            ],
        })
        .width(620.0)
        .height(540.0)
        .bg(tokens.colors.primary_subtle)
        .into()
    }
}
