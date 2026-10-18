use super::button_preview::ButtonPreview;
use super::common::*;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

pub const PATH: &str = "/widgets/button";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Button",
    subtitle: "5 motions",
    glyph: "click",
    tint: style::Tint::Secondary,
};

fission_component! { pub ButtonPage<AnimationGalleryState> {} |_page, _state, _cx| {} => WidgetPage(case(), ButtonPreview()) }

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

const SOURCE: &str = r#"Button(
    id: WidgetId::explicit("send_button"),
    variant: ButtonVariant::Filled,
    child: Text("Send"),
    motion: ButtonMotion::HoverPressRipple,
)"#;

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
