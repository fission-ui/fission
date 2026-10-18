use fission::prelude::*;
use fission::three_d::{Point3D, Primitive3D, Scene3D};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Scene3DEmbedState;

impl GlobalState for Scene3DEmbedState {}

fission_component! {
    pub Scene3DEmbedApp<Scene3DEmbedState>
    |_app, _state, cx| {
        let colours = &cx.view.env().theme.tokens.colors;
        let scene = Scene3D::new()
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
            });
    } =>
    Container(
        Column(
            gap: 16.0,
            children: [
                Text("3D embed").size(28.0),
                Text("A bounded 3D scene composed inside normal UI layout.")
                    .size(14.0)
                    .color(colours.text_secondary),
                Container(scene)
                    .width(480.0)
                    .height(270.0)
                    .border(colours.border, 1.0),
            ],
        ),
    )
    .padding_all(32.0)
}
