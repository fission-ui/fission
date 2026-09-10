//! Browser-rendered visual baselines for the cohesive quality gallery.
//!
//! The suite is ignored because it requires a prebuilt Web test-control bundle,
//! a local HTTP server, and a supported browser. See `platforms/web/README.md` for the
//! compare and intentional-update workflows.

use anyhow::{bail, Context, Result};
use fission_test_driver::{
    compare_png_to_golden, BrowserTestOptions, GoldenOptions, LiveTestClient, SelectorQuery,
    VisibilityState,
};
use std::path::{Path, PathBuf};

const RENDERER: &str = "webgpu-vello";
const GOLDEN_OPTIONS: GoldenOptions = GoldenOptions {
    channel_tolerance: 0,
    max_changed_percent: 0.0,
};
// This case compares two independent rasterizers. The bounded allowance covers
// antialiasing and small glyph-outline differences while retaining a tight
// changed-area budget for layout, spacing, colour, and component regressions.
const GUIDED_FLOW_GOLDEN_OPTIONS: GoldenOptions = GoldenOptions {
    channel_tolerance: 16,
    max_changed_percent: 12.0,
};
const GUIDED_FLOW_MAX_MEAN_ABSOLUTE_ERROR_PERCENT: f64 = 4.0;
const OVERLAY_IDENTIFIERS: [&str; 3] = [
    "quality-gallery.menu.edit-profile",
    "quality-gallery.role.editor",
    "quality-gallery.modal.surface",
];

#[derive(Clone, Copy)]
struct Viewport {
    name: &'static str,
    width: u32,
    height: u32,
}

const VIEWPORTS: [Viewport; 2] = [
    Viewport {
        name: "desktop",
        width: 1280,
        height: 900,
    },
    Viewport {
        name: "narrow",
        width: 390,
        height: 844,
    },
];

#[derive(Clone, Copy)]
struct CaptureState {
    name: &'static str,
    page: &'static str,
    query_value: &'static str,
    visible_identifier: Option<&'static str>,
    interaction: InteractionState,
}

#[derive(Clone, Copy)]
enum InteractionState {
    Rest,
    Focus(&'static str),
    Hover(&'static str),
}

const CAPTURE_STATES: [CaptureState; 6] = [
    CaptureState {
        name: "base",
        page: "quality",
        query_value: "none",
        visible_identifier: None,
        interaction: InteractionState::Rest,
    },
    CaptureState {
        name: "input-focus",
        page: "quality",
        query_value: "none",
        visible_identifier: None,
        interaction: InteractionState::Focus("quality-gallery.input.display-name"),
    },
    CaptureState {
        name: "button-hover",
        page: "quality",
        query_value: "none",
        visible_identifier: None,
        interaction: InteractionState::Hover("quality-gallery.button.primary"),
    },
    CaptureState {
        name: "menu",
        page: "quality",
        query_value: "menu",
        visible_identifier: Some("quality-gallery.menu.edit-profile"),
        interaction: InteractionState::Rest,
    },
    CaptureState {
        name: "select",
        page: "quality",
        query_value: "select",
        visible_identifier: Some("quality-gallery.role.editor"),
        interaction: InteractionState::Rest,
    },
    CaptureState {
        name: "dialog",
        page: "quality",
        query_value: "dialog",
        visible_identifier: Some("quality-gallery.modal.surface"),
        interaction: InteractionState::Rest,
    },
];

const GUIDED_FLOW_STATE: CaptureState = CaptureState {
    name: "guided-flow",
    page: "guided-flow",
    query_value: "none",
    visible_identifier: None,
    interaction: InteractionState::Rest,
};

#[test]
#[ignore = "requires a served Web test-control build and supported browser"]
fn quality_gallery_matches_approved_browser_baselines() -> Result<()> {
    let base_url = std::env::var("FISSION_QUALITY_GALLERY_URL").context(
        "set FISSION_QUALITY_GALLERY_URL to the served widget-gallery Web root; see platforms/web/README.md",
    )?;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden_dir = manifest_dir
        .join("tests")
        .join("goldens")
        .join("quality-gallery")
        .join(RENDERER);
    let artifact_dir = artifact_directory(&manifest_dir);
    let update = std::env::var("FISSION_UPDATE_WIDGET_GALLERY_GOLDENS")
        .ok()
        .as_deref()
        == Some("1");

    std::fs::create_dir_all(&artifact_dir)
        .with_context(|| format!("create artifact directory {}", artifact_dir.display()))?;
    if update {
        std::fs::create_dir_all(&golden_dir)
            .with_context(|| format!("create golden directory {}", golden_dir.display()))?;
    }

    let mut failures = Vec::new();
    let mut captures = Vec::new();

    let guided_viewport = Viewport {
        name: "guided-flow",
        width: 1486,
        height: 1058,
    };
    let guided_case_name = "guided-flow-1486x1058-light".to_string();
    match capture_case(
        &base_url,
        guided_viewport,
        "light",
        GUIDED_FLOW_STATE,
        &artifact_dir,
    ) {
        Ok(actual_png) => {
            let actual_path = artifact_dir.join(format!("{guided_case_name}.actual.png"));
            std::fs::write(&actual_path, &actual_png)
                .with_context(|| format!("write actual image {}", actual_path.display()))?;
            captures.push((guided_case_name, actual_png, actual_path));
        }
        Err(error) => failures.push(format!("guided-flow-1486x1058-light: {error:#}")),
    }

    for viewport in VIEWPORTS {
        for theme in ["light", "dark"] {
            for state in CAPTURE_STATES {
                let case_name = format!(
                    "{}-{}x{}-{theme}-{}",
                    viewport.name, viewport.width, viewport.height, state.name
                );
                match capture_case(&base_url, viewport, theme, state, &artifact_dir) {
                    Ok(actual_png) => {
                        let actual_path = artifact_dir.join(format!("{case_name}.actual.png"));
                        std::fs::write(&actual_path, &actual_png).with_context(|| {
                            format!("write actual image {}", actual_path.display())
                        })?;
                        captures.push((case_name, actual_png, actual_path));
                    }
                    Err(error) => failures.push(format!("{case_name}: {error:#}")),
                }
            }
        }
    }

    if update && failures.is_empty() {
        for (case_name, actual_png, _) in captures {
            // The guided-flow image is an external design oracle, not a
            // self-approved renderer snapshot. Updating ordinary regression
            // baselines must never replace it with the current Fission output.
            if case_name == "guided-flow-1486x1058-light" {
                continue;
            }
            let golden_path = golden_dir.join(format!("{case_name}.png"));
            std::fs::write(&golden_path, actual_png)
                .with_context(|| format!("write golden image {}", golden_path.display()))?;
            println!("updated {case_name}: {}", golden_path.display());
        }
        return Ok(());
    }

    if !update {
        for (case_name, actual_png, actual_path) in captures {
            let golden_path = golden_dir.join(format!("{case_name}.png"));
            if !golden_path.is_file() {
                failures.push(format!(
                    "{case_name}: missing approved baseline {}; actual image: {}",
                    golden_path.display(),
                    actual_path.display()
                ));
            } else {
                compare_case(
                    &case_name,
                    &actual_png,
                    &golden_path,
                    &actual_path,
                    &artifact_dir,
                    &mut failures,
                );
            }
        }
    }

    if !failures.is_empty() {
        bail!(
            "{} visual baseline case(s) failed:\n- {}\nArtifacts: {}\nSet FISSION_UPDATE_WIDGET_GALLERY_GOLDENS=1 only after reviewing an intentional visual change.",
            failures.len(),
            failures.join("\n- "),
            artifact_dir.display()
        );
    }

    Ok(())
}

fn capture_case(
    base_url: &str,
    viewport: Viewport,
    theme: &str,
    state: CaptureState,
    artifact_dir: &Path,
) -> Result<Vec<u8>> {
    let separator = if base_url.contains('?') { '&' } else { '?' };
    let url = format!(
        "{base_url}{separator}page={}&theme={theme}&overlay={}&fission_renderer={RENDERER}",
        state.page, state.query_value
    );
    let mut options = BrowserTestOptions::new(url)
        .fission_canvas()
        .reduced_motion();
    options.viewport_width = viewport.width;
    options.viewport_height = viewport.height;

    let client = LiveTestClient::launch_browser(options)?;
    client.pump()?;
    client.wait_for_idle(10_000, true)?;
    client.pump()?;

    let reduced_motion = client.browser_evaluate_json(
        "globalThis.matchMedia('(prefers-reduced-motion: reduce)').matches",
    )?;
    if reduced_motion.as_bool() != Some(true) {
        bail!("browser did not apply the requested reduced-motion preference");
    }
    let runtime_identity = client.browser_evaluate_json("globalThis.navigator.userAgent")?;
    if let Some(runtime_identity) = runtime_identity.as_str() {
        std::fs::write(
            artifact_dir.join("browser-runtime.txt"),
            format!("{runtime_identity}\n"),
        )
        .context("write browser runtime identity")?;
    }

    let report = client
        .browser_report()
        .context("browser did not provide its readiness report")?;
    if report.renderer.as_deref() != Some(RENDERER) {
        bail!(
            "renderer was {:?}; expected the pinned {RENDERER} renderer",
            report.renderer
        );
    }
    if (report.width, report.height) != (viewport.width, viewport.height) {
        bail!(
            "canvas was {}x{}; expected {}x{}",
            report.width,
            report.height,
            viewport.width,
            viewport.height
        );
    }

    assert_overlay_state(&client, state)?;
    prepare_interaction_state(&client, state.interaction)?;
    client.wait_for_idle(10_000, true)?;
    client.pump()?;
    client.capture_screenshot_png()
}

fn assert_overlay_state(client: &LiveTestClient, expected: CaptureState) -> Result<()> {
    let nodes = client.get_tree()?;
    let is_visible = |identifier: &str| {
        nodes.iter().any(|node| {
            node.identifier.as_deref() == Some(identifier)
                && node.visibility != VisibilityState::Hidden
                && node.visible_bounds.is_some()
        })
    };

    for identifier in OVERLAY_IDENTIFIERS {
        let visible = is_visible(identifier);
        let should_be_visible = expected.visible_identifier == Some(identifier);
        if visible != should_be_visible {
            bail!(
                "overlay semantic node {identifier} visibility was {visible}; expected {should_be_visible} for {} state",
                expected.name
            );
        }
    }
    Ok(())
}

fn prepare_interaction_state(client: &LiveTestClient, state: InteractionState) -> Result<()> {
    let (identifier, focus) = match state {
        InteractionState::Rest => return Ok(()),
        InteractionState::Focus(identifier) => (identifier, true),
        InteractionState::Hover(identifier) => (identifier, false),
    };
    let selector = SelectorQuery::semantic_identifier(identifier);
    client
        .scroll_into_view(selector.clone())
        .with_context(|| format!("scroll interaction target {identifier} into view"))?;
    if focus {
        client
            .focus_selector(selector.clone())
            .with_context(|| format!("focus interaction target {identifier}"))?;
    } else {
        client
            .hover_selector(selector.clone())
            .with_context(|| format!("hover interaction target {identifier}"))?;
    }

    let target = client
        .resolve_selector(selector)
        .with_context(|| format!("resolve interaction target {identifier}"))?;
    if target.visibility == VisibilityState::Hidden || target.visible_bounds.is_none() {
        bail!("interaction target {identifier} was hidden after preparation");
    }
    Ok(())
}

fn compare_case(
    case_name: &str,
    actual_png: &[u8],
    golden_path: &Path,
    actual_path: &Path,
    artifact_dir: &Path,
    failures: &mut Vec<String>,
) {
    if case_name == "guided-flow-1486x1058-light" {
        match mean_absolute_rgb_error_percent(actual_png, golden_path) {
            Ok(error_percent) if error_percent <= GUIDED_FLOW_MAX_MEAN_ABSOLUTE_ERROR_PERCENT => {
                println!("guided-flow mean absolute RGB error: {error_percent:.4}%");
            }
            Ok(error_percent) => failures.push(format!(
                "{case_name}: mean absolute RGB error was {error_percent:.4}% (maximum {GUIDED_FLOW_MAX_MEAN_ABSOLUTE_ERROR_PERCENT:.4}%); expected: {}; actual: {}",
                golden_path.display(),
                actual_path.display()
            )),
            Err(error) => failures.push(format!(
                "{case_name}: mean absolute RGB comparison failed: {error:#}; expected: {}; actual: {}",
                golden_path.display(),
                actual_path.display()
            )),
        }
    }

    let options = if case_name == "guided-flow-1486x1058-light" {
        GUIDED_FLOW_GOLDEN_OPTIONS
    } else {
        GOLDEN_OPTIONS
    };
    match compare_png_to_golden(actual_png, golden_path, Option::<&Path>::None, options) {
        Ok(report) if report.passed(options) => {
            println!("matched {case_name}: {}x{}", report.width, report.height);
        }
        Ok(report) => {
            let diff_path = artifact_dir.join(format!("{case_name}.diff.png"));
            let _ = compare_png_to_golden(actual_png, golden_path, Some(&diff_path), options);
            failures.push(format!(
                "{case_name}: {:.4}% pixels changed ({} of {}, maximum channel delta {}); expected: {}; actual: {}; diff: {}",
                report.changed_percent,
                report.changed_pixels,
                report.total_pixels,
                report.maximum_channel_delta,
                golden_path.display(),
                actual_path.display(),
                diff_path.display()
            ));
        }
        Err(error) => failures.push(format!(
            "{case_name}: comparison failed: {error:#}; expected: {}; actual: {}",
            golden_path.display(),
            actual_path.display()
        )),
    }
}

fn mean_absolute_rgb_error_percent(actual_png: &[u8], golden_path: &Path) -> Result<f64> {
    let actual = image::load_from_memory(actual_png)
        .context("decode actual PNG for mean absolute error")?
        .to_rgb8();
    let expected = image::open(golden_path)
        .with_context(|| format!("decode golden PNG {}", golden_path.display()))?
        .to_rgb8();
    if actual.dimensions() != expected.dimensions() {
        bail!(
            "image dimensions differed: actual {:?}, expected {:?}",
            actual.dimensions(),
            expected.dimensions()
        );
    }

    let absolute_error: u64 = actual
        .as_raw()
        .iter()
        .zip(expected.as_raw())
        .map(|(actual, expected)| actual.abs_diff(*expected) as u64)
        .sum();
    let maximum_error = actual.as_raw().len() as f64 * u8::MAX as f64;
    Ok(absolute_error as f64 * 100.0 / maximum_error)
}

fn artifact_directory(manifest_dir: &Path) -> PathBuf {
    std::env::var_os("FISSION_VISUAL_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            manifest_dir
                .parent()
                .and_then(Path::parent)
                .unwrap_or(manifest_dir)
                .join(".artifacts")
                .join("visual-baselines")
                .join("widget-gallery")
        })
}
