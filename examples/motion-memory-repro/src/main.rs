use fission::motion::{fade, slide_y, Motion, MotionTrack};
use fission::prelude::*;
use image::{ImageBuffer, ImageEncoder, Rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReproScenario {
    Plain,
    PlainImages,
    Motion,
    MotionImages,
    MotionOpacity,
    MotionTranslate,
    StaticOpacity,
}

impl ReproScenario {
    fn from_env() -> Self {
        match std::env::var("FISSION_REPRO_SCENARIO")
            .unwrap_or_else(|_| "motion".to_string())
            .as_str()
        {
            "plain" => Self::Plain,
            "plain-images" => Self::PlainImages,
            "motion-images" => Self::MotionImages,
            "motion-opacity" => Self::MotionOpacity,
            "motion-translate" => Self::MotionTranslate,
            "static-opacity" => Self::StaticOpacity,
            _ => Self::Motion,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::PlainImages => "plain-images",
            Self::Motion => "motion",
            Self::MotionImages => "motion-images",
            Self::MotionOpacity => "motion-opacity",
            Self::MotionTranslate => "motion-translate",
            Self::StaticOpacity => "static-opacity",
        }
    }

    fn uses_motion(self) -> bool {
        matches!(
            self,
            Self::Motion | Self::MotionImages | Self::MotionOpacity | Self::MotionTranslate
        )
    }

    fn uses_images(self) -> bool {
        matches!(self, Self::PlainImages | Self::MotionImages)
    }
}

#[derive(Clone)]
struct MotionMemoryReproApp {
    scenario: ReproScenario,
    rows: usize,
    row_height: f32,
    image_paths: Vec<String>,
    cache_images: bool,
}

impl MotionMemoryReproApp {
    fn from_env() -> anyhow::Result<Self> {
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
        let body: Widget = ReproScreen {
            scenario: app.scenario,
            rows: app.rows,
            row_height: app.row_height,
            image_paths: app.image_paths,
            cache_images: app.cache_images,
        }
        .into();

        let surface = Container::new(body).width(800.0).height(600.0).bg(Color {
            r: 247,
            g: 245,
            b: 240,
            a: 255,
        });

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
        tracks.extend(slide_y(8.0));
    }
    tracks
}

#[derive(Clone)]
struct ReproScreen {
    scenario: ReproScenario,
    rows: usize,
    row_height: f32,
    image_paths: Vec<String>,
    cache_images: bool,
}

impl From<ReproScreen> for Widget {
    fn from(screen: ReproScreen) -> Self {
        Scroll {
            id: Some(WidgetId::explicit("repro_scroll")),
            child: Some(
                Column {
                    gap: Some(8.0),
                    children: (0..screen.rows)
                        .map(|index| {
                            ReproRow {
                                index,
                                scenario: screen.scenario,
                                height: screen.row_height,
                                image_path: screen
                                    .image_paths
                                    .get(index % screen.image_paths.len().max(1))
                                    .cloned(),
                                cache_image: screen.cache_images,
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
            ),
            direction: FlexDirection::Column,
            width: Some(384.0),
            height: Some(600.0),
            show_scrollbar: true,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct ReproRow {
    index: usize,
    scenario: ReproScenario,
    height: f32,
    image_path: Option<String>,
    cache_image: bool,
}

impl From<ReproRow> for Widget {
    fn from(row: ReproRow) -> Self {
        let accent = if row.index % 3 == 0 {
            Color {
                r: 20,
                g: 92,
                b: 116,
                a: 255,
            }
        } else {
            Color {
                r: 184,
                g: 94,
                b: 67,
                a: 255,
            }
        };

        let media: Widget = if let Some(path) = row.image_path {
            let size = row.height - 16.0;
            let image = Image::file(path).size(size, size);
            if row.cache_image {
                image
                    .cache_size(size.ceil() as u32, size.ceil() as u32)
                    .into()
            } else {
                image.into()
            }
        } else {
            Container::new(Spacer {
                width: Some(10.0),
                height: Some(row.height - 16.0),
                ..Default::default()
            })
            .width(10.0)
            .height(row.height - 16.0)
            .bg(accent)
            .border_radius(5.0)
            .into()
        };

        Container::new(Row {
            gap: Some(12.0),
            children: widgets![
                media,
                Column {
                    gap: Some(3.0),
                    children: widgets![
                        Text::new(format!("{} / row {}", row.scenario.label(), row.index + 1))
                            .size(14.0),
                        Text::new("Repeated scroll content to reproduce retained renderer memory.")
                            .size(11.0),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .width(352.0)
        .height(row.height)
        .padding([12.0, 12.0, 8.0, 8.0])
        .bg(Color::WHITE)
        .border(
            Color {
                r: 219,
                g: 215,
                b: 204,
                a: 255,
            },
            1.0,
        )
        .border_radius(14.0)
        .into()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<(), _>::new(MotionMemoryReproApp::from_env()?).run()
}

fn prepare_image_files(count: usize) -> anyhow::Result<Vec<String>> {
    let count = std::env::var("FISSION_REPRO_IMAGE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(count)
        .max(1);
    let pixels = std::env::var("FISSION_REPRO_IMAGE_PIXELS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024u32);
    let dir = std::env::temp_dir().join(format!("fission-motion-memory-repro-{pixels}"));
    std::fs::create_dir_all(&dir)?;

    (0..count)
        .map(|index| {
            let path = dir.join(format!("image-{index:03}.png"));
            if !path.exists() {
                write_repro_image(&path, pixels, index as u32)?;
            }
            Ok(path.to_string_lossy().into_owned())
        })
        .collect()
}

fn write_repro_image(path: &std::path::Path, pixels: u32, seed: u32) -> anyhow::Result<()> {
    let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(pixels, pixels);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let r = ((x.wrapping_add(seed * 17)) % 256) as u8;
        let g = ((y.wrapping_mul(3).wrapping_add(seed * 29)) % 256) as u8;
        let b = ((x.wrapping_add(y).wrapping_add(seed * 41)) % 256) as u8;
        *pixel = Rgba([r, g, b, 255]);
    }

    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    image::codecs::png::PngEncoder::new(writer).write_image(
        image.as_raw(),
        pixels,
        pixels,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(())
}
