use crate::state::CanvasExampleState;
use fission::prelude::*;

fission_component! {
    pub(crate) ViewerScene<CanvasExampleState> { instance: &'static str }
    |scene, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            ZStack(
                id: WidgetId::explicit(&format!(
                    "interactive-viewer.{}.scene",
                    scene.instance,
                )),
                children: [
                    ViewerTile(scene.instance, "origin", 80.0, 80.0, 170.0, 110.0, "Pan anywhere", true),
                    ViewerTile(scene.instance, "zoom", 330.0, 160.0, 190.0, 120.0, "Pinch or zoom", false),
                    ViewerTile(scene.instance, "retained", 160.0, 350.0, 220.0, 100.0, "Retained widgets", false),
                ],
            ),
        )
        .width(620.0)
        .height(540.0)
        .bg(tokens.colors.primary_subtle)
}

fission_component! {
    ViewerTile<CanvasExampleState> {
        instance: &'static str,
        tile_id: &'static str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        label: &'static str,
        primary: bool,
    } |tile, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Positioned(
            id: WidgetId::explicit(&format!(
                "interactive-viewer.{}.tile.{}",
                tile.instance,
                tile.tile_id,
            )),
            left: tile.x,
            top: tile.y,
            width: tile.width,
            height: tile.height,
            child: Container(
                Align(
                    Text(tile.label)
                        .weight(700)
                        .color(if tile.primary {
                            tokens.colors.on_primary
                        } else {
                            tokens.colors.text_primary
                        }),
                ),
            )
            .bg(if tile.primary {
                tokens.colors.primary
            } else {
                tokens.colors.surface_raised
            })
            .border(tokens.colors.border, 1.0)
            .border_radius(18.0),
        )
}
