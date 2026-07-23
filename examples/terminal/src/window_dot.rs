use fission::prelude::*;

const DOT_SIZE: f32 = 12.0;

pub struct WindowDot {
    pub color: Color,
}

impl From<WindowDot> for Widget {
    fn from(dot: WindowDot) -> Self {
        let (_, view) = fission::build::current::<()>();

        Container::new(Spacer {
            width: Some(DOT_SIZE),
            height: Some(DOT_SIZE),
            ..Default::default()
        })
        .size(DOT_SIZE, DOT_SIZE)
        .bg(dot.color)
        .border_radius(view.env().theme.tokens.radii.full)
        .into()
    }
}
