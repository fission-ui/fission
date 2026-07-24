use crate::fixtures::prepare_image_files;
use crate::layout::{REPRO_SURFACE_HEIGHT, REPRO_SURFACE_WIDTH, ROUTE_TRANSLATE_DISTANCE};
use crate::palette::REPRO_BACKGROUND;
use crate::repro_screen::ReproScreen;
use crate::scenario::ReproScenario;
use fission::motion::{fade, slide_y, Motion, MotionTrack};
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct MotionMemoryReproApp {
    scenario: ReproScenario,
    rows: usize,
    row_height: f32,
    image_paths: Vec<String>,
    cache_images: bool,
}

impl MotionMemoryReproApp {
    pub(crate) fn from_env() -> anyhow::Result<Self> {
        let scenario = ReproScenario::from_env();
        let rows = std::env::var("FISSION_REPRO_ROWS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(48);
        let row_height = std::env::var("FISSION_REPRO_ROW_HEIGHT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(if scenario.uses_images() { 96.0 } else { 48.0 });
        let image_paths = if scenario.uses_images() {
            prepare_image_files(rows)?
        } else {
            Vec::new()
        };

        Ok(Self {
            scenario,
            rows,
            row_height,
            image_paths,
            cache_images: std::env::var("FISSION_REPRO_CACHE_IMAGES").is_ok(),
        })
    }
}

impl From<MotionMemoryReproApp> for Widget {
    fn from(app: MotionMemoryReproApp) -> Self {
        let (_ctx, _view) = fission::build::current::<()>();
        let body: Widget = ReproScreen {
            scenario: app.scenario,
            rows: app.rows,
            row_height: app.row_height,
            image_paths: app.image_paths,
            cache_images: app.cache_images,
        }
        .into();

        let surface = Container::new(body)
            .width(REPRO_SURFACE_WIDTH)
            .height(REPRO_SURFACE_HEIGHT)
            .bg(REPRO_BACKGROUND);

        if app.scenario.uses_motion() {
            let tracks = match app.scenario {
                ReproScenario::MotionOpacity => route_tracks(true, false),
                ReproScenario::MotionTranslate => route_tracks(false, true),
                _ => route_tracks(true, true),
            };
            return Motion {
                id: WidgetId::explicit("repro_route_motion"),
                tracks,
                child: surface.into(),
                ..Default::default()
            }
            .into();
        }

        match app.scenario {
            ReproScenario::Plain => surface.into(),
            ReproScenario::PlainImages => surface.into(),
            ReproScenario::StaticOpacity => Composite::new(surface).opacity(1.0).into(),
            ReproScenario::Motion
            | ReproScenario::MotionImages
            | ReproScenario::MotionOpacity
            | ReproScenario::MotionTranslate => unreachable!("motion scenarios return above"),
        }
    }
}

fn route_tracks(with_opacity: bool, with_translate: bool) -> Vec<MotionTrack> {
    let mut tracks = Vec::new();
    if with_opacity {
        tracks.extend(fade());
    }
    if with_translate {
        tracks.extend(slide_y(ROUTE_TRANSLATE_DISTANCE));
    }
    tracks
}
