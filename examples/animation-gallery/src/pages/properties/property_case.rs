use crate::style::{color, BLUE, PINK, TEAL, VIOLET};
use fission::motion::{
    self, deg, px, scalar, MotionEasing, MotionExpr, MotionPhase, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition,
};
use fission::op::Color;

#[derive(Clone)]
pub struct PropertyCase {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub property_name: &'static str,
    pub value_type: &'static str,
    pub phase: &'static str,
    pub layout: &'static str,
    pub paint: &'static str,
    pub reduced: &'static str,
    pub notes: &'static str,
    pub demo_label: &'static str,
    pub color: Color,
    pub track: MotionTrack,
    pub track_source: &'static str,
}

pub(super) fn property_case(path: &str) -> PropertyCase {
    match path {
        "/properties/translate" => case(
            "property.translate",
            "Translate X/Y",
            "Composite translation moves visuals without changing layout.",
            "translate_x / translate_y",
            "px",
            "Composite",
            "No",
            "No",
            "Usually disabled or shortened",
            "Good for entrance and shared-position motion.",
            "Slide",
            BLUE,
            MotionTrack::composite(
                MotionPropertyId::TranslateX,
                MotionStartValue::Explicit(px(-32.0)),
                px(0.0),
            ),
            PROPERTY_TRANSLATE_SOURCE,
        ),
        "/properties/scale" => case(
            "property.scale",
            "Scale",
            "Composite scale changes visual size without reflow.",
            "scale",
            "number",
            "Composite",
            "No",
            "No",
            "Often reduced to fade",
            "Use sparingly for popovers, modals, and press feedback.",
            "Scale",
            VIOLET,
            MotionTrack::composite(
                MotionPropertyId::Scale,
                MotionStartValue::Explicit(scalar(0.86)),
                scalar(1.0),
            ),
            PROPERTY_SCALE_SOURCE,
        ),
        "/properties/rotation" => case(
            "property.rotation",
            "Rotation",
            "Composite rotation for indicators and loaders.",
            "rotation",
            "deg",
            "Composite",
            "No",
            "No",
            "Usually disabled",
            "Useful for chevrons and indeterminate progress.",
            "Rotate",
            PINK,
            MotionTrack::composite(
                MotionPropertyId::Rotation,
                MotionStartValue::Explicit(deg(-8.0)),
                deg(8.0),
            ),
            PROPERTY_ROTATION_SOURCE,
        ),
        "/properties/size" => case(
            "property.height",
            "Width / Height",
            "Layout motion affects measurement, invalidation, and clipping.",
            "height",
            "px / intrinsic",
            "Layout",
            "Yes",
            "No",
            "Partial",
            "Accordion/collapse motion should clip the animated panel.",
            "Height",
            TEAL,
            MotionTrack {
                property: MotionPropertyId::Height,
                phase: MotionPhase::Layout,
                from: MotionStartValue::Explicit(px(0.0)),
                to: MotionExpr::IntrinsicHeight,
                transition: MotionTransition::spring(150.0, 22.0),
            },
            PROPERTY_HEIGHT_SOURCE,
        ),
        "/properties/background-color" => color_case(
            "Background Color",
            MotionPropertyId::BackgroundColor,
            "background_color",
        ),
        "/properties/border-color" => color_case(
            "Border Color",
            MotionPropertyId::BorderColor,
            "border_color",
        ),
        "/properties/corner-radius" => case(
            "property.corner_radius",
            "Corner Radius",
            "Paint/layout radius motion for shape transitions.",
            "corner_radius",
            "px",
            "Paint",
            "No",
            "Yes",
            "Yes",
            "Shape motion should remain subtle and deterministic.",
            "Radius",
            BLUE,
            MotionTrack {
                property: MotionPropertyId::CornerRadius,
                phase: MotionPhase::Paint,
                from: MotionStartValue::Explicit(px(4.0)),
                to: px(24.0),
                transition: MotionTransition::tween(180, MotionEasing::EaseOut),
            },
            PROPERTY_RADIUS_SOURCE,
        ),
        "/properties/clip-reveal" => case(
            "property.clip",
            "Clip / Reveal",
            "Reveal combines clipping with layout or composite tracks.",
            "height + clip",
            "px",
            "Layout + Composite",
            "Yes",
            "No",
            "Partial",
            "The wrapper clips while height or translate changes.",
            "Reveal",
            TEAL,
            MotionTrack {
                property: MotionPropertyId::Height,
                phase: MotionPhase::Layout,
                from: MotionStartValue::Explicit(px(0.0)),
                to: MotionExpr::IntrinsicHeight,
                transition: MotionTransition::tween(180, MotionEasing::EaseOut),
            },
            PROPERTY_CLIP_SOURCE,
        ),
        _ => case(
            "property.opacity",
            "Opacity",
            "Composite opacity is the safest default visual motion.",
            "opacity",
            "number [0..1]",
            "Composite",
            "No",
            "Yes",
            "Fades only",
            "Opacity is compositor friendly when supported by the shell.",
            "Opacity",
            BLUE,
            MotionTrack::composite(
                MotionPropertyId::Opacity,
                MotionStartValue::Explicit(scalar(0.0)),
                scalar(1.0),
            ),
            PROPERTY_OPACITY_SOURCE,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn case(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    property_name: &'static str,
    value_type: &'static str,
    phase: &'static str,
    layout: &'static str,
    paint: &'static str,
    reduced: &'static str,
    notes: &'static str,
    demo_label: &'static str,
    color: Color,
    track: MotionTrack,
    track_source: &'static str,
) -> PropertyCase {
    PropertyCase {
        id,
        title,
        description,
        property_name,
        value_type,
        phase,
        layout,
        paint,
        reduced,
        notes,
        demo_label,
        color,
        track,
        track_source,
    }
}

fn color_case(title: &'static str, property: MotionPropertyId, name: &'static str) -> PropertyCase {
    case(
        name,
        title,
        "Paint color motion is explicit and inspectable.",
        name,
        "Color",
        "Paint",
        "No",
        "Yes",
        "Yes",
        "Color interpolation is typed and cannot be confused with scalar motion.",
        "Color",
        PINK,
        MotionTrack {
            property,
            phase: MotionPhase::Paint,
            from: MotionStartValue::Explicit(motion::color(color(120, 95, 255, 255))),
            to: motion::color(color(15, 160, 172, 255)),
            transition: MotionTransition::tween(180, MotionEasing::EaseOut),
        },
        PROPERTY_COLOR_SOURCE,
    )
}

const PROPERTY_OPACITY_SOURCE: &str =
    r#"MotionTrack::composite(MotionPropertyId::Opacity, scalar(0.0), scalar(1.0))"#;
const PROPERTY_TRANSLATE_SOURCE: &str =
    r#"MotionTrack::composite(MotionPropertyId::TranslateX, px(-32.0), px(0.0))"#;
const PROPERTY_SCALE_SOURCE: &str =
    r#"MotionTrack::composite(MotionPropertyId::Scale, scalar(0.86), scalar(1.0))"#;
const PROPERTY_ROTATION_SOURCE: &str =
    r#"MotionTrack::composite(MotionPropertyId::Rotation, deg(-8.0), deg(8.0))"#;
const PROPERTY_HEIGHT_SOURCE: &str = r#"MotionTrack { property: MotionPropertyId::Height, phase: MotionPhase::Layout, to: MotionExpr::IntrinsicHeight, ... }"#;
const PROPERTY_COLOR_SOURCE: &str = r#"MotionTrack { property: MotionPropertyId::BackgroundColor, phase: MotionPhase::Paint, from: color(start), to: color(end), ... }"#;
const PROPERTY_RADIUS_SOURCE: &str = r#"MotionTrack { property: MotionPropertyId::CornerRadius, phase: MotionPhase::Paint, from: px(4), to: px(24), ... }"#;
const PROPERTY_CLIP_SOURCE: &str = r#"Motion { id, tracks: collapse_y(), clip_to_bounds: true, child, ..Default::default() }.into()"#;
