use crate::layout::COLOR_DOT_SIZE;
use fission::prelude::*;

pub(super) struct ColorDot {
    pub color: Color,
}

impl From<ColorDot> for Widget {
    fn from(dot: ColorDot) -> Self {
        Container::new(Text::new(" "))
            .width(COLOR_DOT_SIZE)
            .height(COLOR_DOT_SIZE)
            .border_radius(COLOR_DOT_SIZE / 2.0)
            .bg(dot.color)
            .into()
    }
}
