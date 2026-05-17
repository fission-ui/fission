use fission::prelude::*;
use fission_3d::{Point3D, Primitive3D, Scene3D};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Scene3DEmbedState;

impl AppState for Scene3DEmbedState {}

pub struct Scene3DEmbedApp;

impl Widget<Scene3DEmbedState> for Scene3DEmbedApp {
    fn build(
        &self,
        _ctx: &mut BuildCtx<Scene3DEmbedState>,
        view: &View<Scene3DEmbedState>,
    ) -> Node {
        let tokens = &view.env.theme.tokens.colors;
        Container::new(
            Column {
                gap: Some(16.0),
                children: vec![
                    Text::new("3D embed").size(28.0).into_node(),
                    Text::new("A bounded 3D scene composed inside normal UI layout.")
                        .size(14.0)
                        .color(tokens.text_secondary)
                        .into_node(),
                    Container::new(
                        Scene3D::new()
                            .width(480.0)
                            .height(270.0)
                            .add_primitive(Primitive3D::Cube {
                                center: Point3D::new(0.0, 0.0, 0.0),
                                size: 2.5,
                                color: Color {
                                    r: 20,
                                    g: 184,
                                    b: 166,
                                    a: 255,
                                },
                            })
                            .build(_ctx, view),
                    )
                    .width(480.0)
                    .height(270.0)
                    .border(tokens.border, 1.0)
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(32.0)
        .into_node()
    }
}
