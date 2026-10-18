use super::carousel_preview::CarouselPreview;
use super::common::*;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

pub const PATH: &str = "/widgets/carousel";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Carousel",
    subtitle: "custom",
    glyph: "slides",
    tint: style::Tint::Success,
};

fission_component! { pub CarouselPage<AnimationGalleryState> {} |_page, _state, _cx| {} => WidgetPage(case(), CarouselPreview()) }

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

const SOURCE: &str = r#"fission_component! {
    pub AppCarousel<()> { active_index: usize, slides: Vec<Widget> }
    |carousel, _state, cx| {} =>
        Row(
            gap: cx.view.env().theme.tokens.spacing.s,
            children: [
                for (i, slide) in carousel.slides.into_iter().enumerate() {
                    Motion(
                    id: WidgetId::derived(WidgetId::explicit("carousel").as_u128(), &[i as u32]),
                    tracks: vec![slide_translate_x(carousel.active_index, i)],
                    child: slide,
                    key: i,
                )
                },
            ],
        )
}"#;
