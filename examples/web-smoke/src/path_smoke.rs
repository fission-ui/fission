use fission::prelude::*;

const SCENE_WIDTH: f32 = 184.0;
const SCENE_HEIGHT: f32 = 112.0;
const PATH_WIDTH: f32 = 160.0;
const PATH_HEIGHT: f32 = 96.0;
const PATH_DATA: &str =
    "M-20 50 C10 12 35 12 60 50 C85 88 110 88 135 50 C155 20 175 20 180 50 L180 110 L-20 110 Z";

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScenePathSmoke;

impl From<ScenePathSmoke> for Widget {
    fn from(_scene_path: ScenePathSmoke) -> Self {
        let path_id = SceneNodeId::from_key(&0x5741_5645_u64);
        let mut scene = Scene2D::new();
        scene.clear(Color {
            r: 17,
            g: 29,
            b: 44,
            a: 255,
        });
        scene.path(
            path_id.clone(),
            PATH_DATA,
            Bounds2D::from_top_left(
                Place::new(Px(12.0), Px(8.0)),
                Size::new(Px(PATH_WIDTH), Px(PATH_HEIGHT)),
            ),
            Some(ir_op::Fill::Solid(Color {
                r: 0,
                g: 180,
                b: 216,
                a: 255,
            })),
            Some(ir_op::Stroke {
                fill: ir_op::Fill::Solid(Color {
                    r: 247,
                    g: 251,
                    b: 255,
                    a: 255,
                }),
                width: 4.0,
                dash_array: None,
                line_cap: ir_op::LineCap::Round,
                line_join: ir_op::LineJoin::Round,
            }),
            Layer(1),
        );

        Scene2DView::new(scene.finish(Tick(0)), SCENE_WIDTH, SCENE_HEIGHT)
            .object_actions(
                path_id,
                SceneObjectActions::new("Browser renderer path sample")
                    .semantics_identifier("web-smoke.scene-path"),
            )
            .into()
    }
}
