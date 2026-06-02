use fission::prelude::*;
use fission::three_d::{Point3D, Primitive3D, Scene3D};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Scene3DEmbedState;

impl GlobalState for Scene3DEmbedState {}

#[derive(Clone)]
pub struct Scene3DEmbedApp;

impl From<Scene3DEmbedApp> for Widget {
    fn from(_component: Scene3DEmbedApp) -> Self {
        let (_ctx, view) = fission::build::current::<Scene3DEmbedState>();
        let tokens = &view.env().theme.tokens.colors;
        Container::new(Column {
            gap: Some(16.0),
            children: vec![
                Text::new("3D embed").size(28.0).into(),
                Text::new("A bounded 3D scene composed inside normal UI layout.")
                    .size(14.0)
                    .color(tokens.text_secondary)
                    .into(),
                Container::new(Scene3D::new().width(480.0).height(270.0).add_primitive(
                    Primitive3D::Cube {
                        center: Point3D::new(0.0, 0.0, 0.0),
                        size: 2.5,
                        color: Color {
                            r: 20,
                            g: 184,
                            b: 166,
                            a: 255,
                        },
                    },
                ))
                .width(480.0)
                .height(270.0)
                .border(tokens.border, 1.0)
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(32.0)
        .into()
    }
}
