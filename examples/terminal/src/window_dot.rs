use fission::prelude::*;

const DOT_SIZE: f32 = 12.0;

fission_component! {
    pub WindowDot<()> { color: Color } |dot, _state, cx| {} =>
        Container(Spacer(width: DOT_SIZE, height: DOT_SIZE))
        .size(DOT_SIZE, DOT_SIZE)
        .bg(dot.color)
        .border_radius(cx.view.env().theme.tokens.radii.full)
}
