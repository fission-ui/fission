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
                StripItem(
                    "Typed & discoverable",
                    "via widget-owned enums",
                    key: "typed",
                ),
                StripItem(
                    "Composability by design",
                    "Additive + last-wins",
                    key: "composable",
                ),
                StripItem(
                    "One runtime",
                    "Everything lowers to MotionExpr",
                    key: "runtime",
                ),
                StripItem(
                    "Deterministic",
                    "Pure, replayable, testable",
                    key: "deterministic",
                ),
                StripItem(
                    "Accessibility first",
                    "Policy respects users",
                    key: "accessibility",
                ),
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
