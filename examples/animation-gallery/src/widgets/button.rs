use super::button_preview::ButtonPreview;
use super::common::*;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_VIOLET;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/button";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Button",
    subtitle: "5 motions",
    glyph: "click",
    tint: SOFT_VIOLET,
};

pub struct ButtonPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ButtonPage<'_>> for Widget {
    fn from(page: ButtonPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: ButtonPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

fn case() -> GalleryCase {
    GalleryCase {
        title: "Button",
        description: "Hover, press, and ripple feedback via ButtonMotion.",
        motions: BUTTON_MOTIONS,
        slots: &["root", "ripple"],
        tracks: &["root.scale", "ripple.spawn"],
        exprs: &["MotionPredicate::Hovered", "MotionPredicate::Pressed"],
        ergonomic_source: SOURCE,
        native_source: NATIVE_SOURCE,
        declaration_source: DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic:
            "ButtonMotion composes hover, press, and ripple atoms without rebuilding the button tree.",
    }
}

const SOURCE: &str = r#"Button {
    id: Some(WidgetId::explicit("send_button")),
    variant: ButtonVariant::Filled,
    child: Some(Text::new("Send").into()),
    motion: Some(ButtonMotion::HoverPressRipple),
    ..Default::default()
}.into()"#;

const NATIVE_SOURCE: &str = r#"Motion {
    id: WidgetId::explicit("send_button_motion"),
    tracks: vec![MotionTrack::composite(
        MotionPropertyId::Scale,
        MotionStartValue::Current,
        MotionExpr::If { predicate: MotionPredicate::Pressed(send_button), ... },
    )],
    child: Button { motion: None, ..button }.into(),
    ..Default::default()
}.into()"#;

const DECLARATION_SOURCE: &str = r#"MotionDeclaration {
    id: WidgetId::derived(send_button.as_u128(), &[root_motion]),
    kind: MotionDeclarationKind::Tracks { tracks: vec![root.scale] },
}
MotionDeclaration {
    id: WidgetId::derived(send_button, [ripple]),
    kind: MotionDeclarationKind::RippleLayer(ripple_effect()),
}"#;
