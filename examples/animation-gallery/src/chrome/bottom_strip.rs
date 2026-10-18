use super::*;
use crate::style;

fission_component! {
    pub(super) BottomStrip<AnimationGalleryState> |_strip, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.l,
            children: [
                StripItem { title: "Typed & discoverable", body: "via widget-owned enums" },
                StripItem { title: "Composability by design", body: "Additive + last-wins" },
                StripItem { title: "One runtime", body: "Everything lowers to MotionExpr" },
                StripItem { title: "Deterministic", body: "Pure, replayable, testable" },
                StripItem { title: "Accessibility first", body: "Policy respects users" },
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
