use super::*;

pub(super) struct BottomStrip;

impl From<BottomStrip> for Widget {
    fn from(_: BottomStrip) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Wrap {
            direction: FlexDirection::Row,
            spacing: Some(tokens.spacing.l),
            children: vec![
                StripItem {
                    title: "Typed & discoverable",
                    body: "via widget-owned enums",
                }
                .into(),
                StripItem {
                    title: "Composability by design",
                    body: "Additive + last-wins",
                }
                .into(),
                StripItem {
                    title: "One runtime",
                    body: "Everything lowers to MotionExpr",
                }
                .into(),
                StripItem {
                    title: "Deterministic",
                    body: "Pure, replayable, testable",
                }
                .into(),
                StripItem {
                    title: "Accessibility first",
                    body: "Policy respects users",
                }
                .into(),
            ],
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
