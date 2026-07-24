use super::carousel_preview::CarouselPreview;
use super::common::*;
use crate::state::AnimationGalleryState;
use crate::style::SOFT_TEAL;
use fission::build::BuildCtxHandle;
use fission::Widget;

pub const PATH: &str = "/widgets/carousel";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Carousel",
    subtitle: "custom",
    glyph: "slides",
    tint: SOFT_TEAL,
};

pub struct CarouselPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<CarouselPage<'_>> for Widget {
    fn from(page: CarouselPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: CarouselPreview { state: page.state }.into(),
        }
        .into()
    }
}

fn case() -> GalleryCase {
    GalleryCase {
        title: "Carousel",
        description: "Composite custom widget pattern for paged content motion.",
        motions: DIRECTIONAL_MOTIONS,
        slots: &["viewport", "slide"],
        tracks: &["slide.translate_x", "slide.opacity"],
        exprs: &[
            "MotionExpr::LayoutX(active_slide)",
            "MotionStartValue::Current",
        ],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic:
            "Carousel motion is explicit page-state motion, not autoplaying gallery chrome.",
    }
}

const SOURCE: &str = r#"pub struct AppCarousel {
    pub active_index: usize,
    pub slides: Vec<Widget>,
}

impl From<AppCarousel> for Widget {
    fn from(carousel: AppCarousel) -> Self {
        let (_, view) = fission::build::current::<()>();
        Row {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: carousel.slides.into_iter().enumerate().map(|(i, slide)| {
                Motion {
                    id: WidgetId::derived(WidgetId::explicit("carousel").as_u128(), &[i as u32]),
                    tracks: vec![slide_translate_x(carousel.active_index, i)],
                    child: slide,
                    ..Default::default()
                }.into()
            }).collect(),
            ..Default::default()
        }.into()
    }
}"#;
