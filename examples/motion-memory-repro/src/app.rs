use crate::fixtures::prepare_image_files;
use crate::layout::{REPRO_SURFACE_HEIGHT, REPRO_SURFACE_WIDTH, ROUTE_TRANSLATE_DISTANCE};
use crate::palette::REPRO_BACKGROUND;
use crate::repro_screen::ReproScreen;
use crate::scenario::ReproScenario;
use fission::motion::{fade, slide_y, Motion, MotionTrack};
use fission::prelude::*;

fission_component! {
pub MotionMemoryReproApp<()> {
    scenario: ReproScenario,
    rows: usize,
    row_height: f32,
    image_paths: Vec<String>,
    cache_images: bool,
} |app, _state, _cx| {
} => if app.scenario.uses_motion() {
        Motion(
            id: WidgetId::explicit("repro_route_motion"),
            tracks: match app.scenario {
                ReproScenario::MotionOpacity => route_tracks(true, false),
                ReproScenario::MotionTranslate => route_tracks(false, true),
                _ => route_tracks(true, true),
            },
            child: ReproSurface(app.scenario, app.rows, app.row_height, app.image_paths.clone(), app.cache_images),
        )
    } else if app.scenario == ReproScenario::StaticOpacity {
        Composite::new(
                ReproSurface::new(app.scenario, app.rows, app.row_height, app.image_paths.clone(), app.cache_images)
            ).opacity(1.0)
    } else {
        ReproSurface(app.scenario, app.rows, app.row_height, app.image_paths.clone(), app.cache_images)
    }
}

fission_component! {
    ReproSurface<()> {
        scenario: ReproScenario,
        rows: usize,
        row_height: f32,
        image_paths: Vec<String>,
        cache_images: bool,
    } |surface, _state, _cx| {} =>
    Container(
        ReproScreen(
            surface.scenario,
            surface.rows,
            surface.row_height,
            surface.image_paths.clone(),
            surface.cache_images,
        )
    )
    .width(REPRO_SURFACE_WIDTH)
    .height(REPRO_SURFACE_HEIGHT)
    .bg(REPRO_BACKGROUND)
}

impl MotionMemoryReproApp {
    pub fn from_env() -> anyhow::Result<Self> {
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

        Ok(Self::new(
            scenario,
            rows,
            row_height,
            image_paths,
            std::env::var("FISSION_REPRO_CACHE_IMAGES").is_ok(),
        ))
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
