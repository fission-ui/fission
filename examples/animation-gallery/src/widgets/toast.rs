use super::common::*;
use super::toast_preview::ToastPreview;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/toast";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Toast",
    subtitle: "5 motions",
    glyph: "toast",
    tint: SOFT_TEAL,
};

pub struct ToastPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ToastPage<'_>> for Widget {
    fn from(page: ToastPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: ToastPreview {
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
        title: "Toast",
        description: "Presence-driven notification motion for surface enter and exit.",
        motions: DIRECTIONAL_MOTIONS,
        slots: &["surface"],
        tracks: &["surface.translate_y", "surface.opacity"],
        exprs: &["px(-18) -> px(0)", "scalar(0) -> scalar(1)"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Toast motion is presence-based; app code owns lifetime and dismissal.",
    }
}

const SOURCE: &str = r#"Toast {
    id: WidgetId::explicit("saved_toast"),
    kind: ToastKind::Success,
    message: "Saved".into(),
    motion: Some(ToastMotion::SlideFromTop + ToastMotion::Fade),
    ..toast
}.into()"#;
