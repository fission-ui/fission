use super::common::*;
use crate::state::{current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice};
use crate::style::{BLUE, SOFT_TEAL, TEAL, VIOLET};
use fission::build::BuildCtxHandle;
use fission::motion::{px, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::{Container, Row, Text, Widget, WidgetId};

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

struct CarouselPreview<'a> {
    state: &'a AnimationGalleryState,
}

impl From<CarouselPreview<'_>> for Widget {
    fn from(preview: CarouselPreview<'_>) -> Self {
        let state = preview.state;
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
        let offset = carousel_offset(state);
        PreviewShell {
            child: Row {
                gap: Some(10.0),
                children: vec![
                    Tile {
                        label: "One",
                        bg: BLUE,
                    }
                    .into(),
                    Motion {
                        id: WidgetId::explicit("gallery.carousel.slide"),
                        tracks: vec![MotionTrack::composite(
                            MotionPropertyId::TranslateX,
                            MotionStartValue::Explicit(px(offset)),
                            px(offset * (1.0 - progress)),
                        )],
                        child: Tile {
                            label: "Two",
                            bg: TEAL,
                        }
                        .into(),
                        ..Default::default()
                    }
                    .into(),
                    Tile {
                        label: "Three",
                        bg: VIOLET,
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn carousel_offset(state: &AnimationGalleryState) -> f32 {
    if state.motion != MotionChoice::Composition {
        return 18.0;
    }
    let atoms = current_composition_atoms(state);
    if atoms
        .iter()
        .any(|atom| matches!(atom, MotionAtom::FromLeft))
    {
        -18.0
    } else if atoms
        .iter()
        .any(|atom| matches!(atom, MotionAtom::FromRight))
    {
        18.0
    } else {
        0.0
    }
}

struct Tile<'a> {
    label: &'a str,
    bg: fission::op::Color,
}

impl From<Tile<'_>> for Widget {
    fn from(tile: Tile<'_>) -> Self {
        Container::new(
            Text::new(tile.label)
                .size(13.0)
                .color(fission::op::Color::WHITE),
        )
        .width(110.0)
        .height(86.0)
        .padding_all(24.0)
        .border_radius(16.0)
        .bg(tile.bg)
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
        Row {
            gap: Some(10.0),
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
