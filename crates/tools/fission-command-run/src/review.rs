//! Owned finite website review; stdout JSON is exactly one typed report.
use crate::{
    absolute_path,
    preview::{self, PreviewOptions},
    WebCargoOptions,
};
use anyhow::{bail, Context, Result};
use fission_command_core::Target;
use fission_command_process::{in_owned_process_tree, run_captured, CleanupError, ProcessSession};
use fission_command_site::preview::normalize_mount;
use fission_test_driver::{
    browser::review::{review_case, BrowserReview},
    BrowserTestOptions,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    time::Duration,
};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}
impl std::str::FromStr for Viewport {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        let (w, h) = value
            .split_once('x')
            .context("viewport must be WIDTHxHEIGHT CSS pixels")?;
        let v = Self {
            width: w.parse()?,
            height: h.parse()?,
        };
        if !(200..=4096).contains(&v.width) || !(200..=4096).contains(&v.height) {
            bail!("viewport dimensions must be 200..=4096");
        }
        Ok(v)
    }
}
#[derive(Clone, Debug)]
pub struct ReviewOptions {
    pub project_dir: PathBuf,
    pub target: Target,
    pub output_dir: PathBuf,
    pub routes: Vec<String>,
    pub viewports: Vec<Viewport>,
    pub mount: String,
    pub release: bool,
    pub web_cargo: WebCargoOptions,
    pub port: u16,
    pub startup_timeout: Duration,
    pub case_timeout: Duration,
    pub json: bool,
    pub strict: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Coverage {
    pub source: String,
    pub discovered: Option<Vec<String>>,
    pub selected: Vec<String>,
    pub undiscovered_routes_reviewed: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCase {
    pub route: String,
    pub requested_url: String,
    pub viewport: Viewport,
    pub status: String,
    pub result: BrowserReview,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Failure {
    pub stage: String,
    pub evidence: String,
    pub recovery: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewReport {
    pub schema: String,
    pub target: Target,
    pub mount: String,
    pub base_url: Option<String>,
    pub output_dir: PathBuf,
    pub device_scale_factor: u32,
    pub concurrency: u32,
    pub coverage: Coverage,
    pub viewports: Vec<Viewport>,
    pub cases: Vec<ReviewCase>,
    pub status: String,
    pub exit_code: i32,
    pub failure: Option<Failure>,
    pub limitations: Vec<String>,
    pub owned_resources_released: bool,
    pub strict_warnings: bool,
    pub image_inspection_required: bool,
}
#[derive(Debug)]
pub struct ReviewExit(pub i32);
impl std::fmt::Display for ReviewExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "website review exited {}; inspect review.json and screenshots",
            self.0
        )
    }
}
impl std::error::Error for ReviewExit {}

pub fn run(mut o: ReviewOptions) -> Result<()> {
    let mut report=ReviewReport {
        schema:"fission.website-review.v1".into(),target:o.target,mount:o.mount.clone(),
        base_url:None,output_dir:o.output_dir.clone(),device_scale_factor:1,concurrency:1,
        coverage:Coverage {source:"unselected".into(), discovered:None,selected:Vec::new(),undiscovered_routes_reviewed:false},
        viewports:Vec::new(),cases:Vec::new(),status:"execution_failed".into(),exit_code:1,failure:None,
        limitations:vec!["Screenshots require human/agent image inspection. Nonuniform pixels demonstrate capture content, not visual correctness. Structural checks are partial; this is not an accessibility, SEO or exhaustive clipping audit.".into()],owned_resources_released:true,
        strict_warnings:o.strict,image_inspection_required:true,
    };
    let result = (|| {
        o.output_dir = absolute_path(&o.output_dir)?;
        report.output_dir = o.output_dir.clone();
        fs::create_dir_all(&o.output_dir).context("output: cannot create review directory")?;
        o.mount = normalize_mount(&o.mount)?;
        report.mount = o.mount.clone();
        if !matches!(o.target, Target::Web | Target::Site) {
            bail!("validation: review supports only browser website targets");
        }
        if o.case_timeout.is_zero() || o.case_timeout > Duration::from_secs(300) {
            bail!("validation: case timeout must be 1..=300 seconds");
        }
        o.project_dir = absolute_path(&o.project_dir)?;
        let discovered = if o.target == Target::Site {
            fission_command_site::review_routes(&o.project_dir)?
        } else {
            None
        };
        let mut routes = if o.routes.is_empty() {
            {
                let mut selected = discovered.clone().unwrap_or_default();
                selected.push("/".into());
                selected
            }
        } else {
            o.routes.clone()
        };
        for route in &mut routes {
            *route = normalize_route(route)?;
        }
        routes.sort();
        routes.dedup();
        if routes.is_empty() || routes.len() > 64 {
            bail!("coverage: select 1..=64 routes");
        }
        if o.viewports.is_empty() {
            o.viewports = vec![
                Viewport {
                    width: 390,
                    height: 900,
                },
                Viewport {
                    width: 800,
                    height: 900,
                },
                Viewport {
                    width: 1280,
                    height: 900,
                },
            ];
        }
        o.viewports.sort_by_key(|v| (v.width, v.height));
        o.viewports.dedup();
        if o.viewports.len() > 16 || routes.len() * o.viewports.len() > 256 {
            bail!("coverage: matrix limit is 256 cases and 16 viewports");
        }
        report.viewports = o.viewports.clone();
        report.coverage = Coverage {
            source: if !o.routes.is_empty() {
                "explicit"
            } else if discovered.is_some() {
                "static_content_metadata"
            } else {
                "root_only"
            }
            .into(),
            discovered,
            selected: routes.clone(),
            undiscovered_routes_reviewed: false,
        };
        if report.coverage.discovered.is_none() {
            report.limitations.push("Route discovery unavailable for Web/custom site builders. Only selected routes were reviewed; use --route for every required route.".into());
        }
        let session = ProcessSession::new()?;
        let mut cancelled = || session.interrupted();
        let preview_result = preview::with_preview(
            PreviewOptions {
                project_dir: o.project_dir.clone(),
                target: o.target,
                release: o.release,
                web_cargo: o.web_cargo.clone(),
                host: "127.0.0.1".into(),
                port: o.port,
                mount: o.mount.clone(),
                entry: "index.html".into(),
                startup_timeout: o.startup_timeout,
                live_test: o.target == Target::Web,
                json: true,
                stdin_control: false,
                open: false,
            },
            &mut cancelled,
            |_, base, server, cancelled| {
                report.base_url = Some(base.to_string());
                for (ri, route) in routes.iter().enumerate() {
                    for viewport in &o.viewports {
                        let url = route_url(base, route)?;
                        let name = artifact_name(ri, *viewport);
                        let screenshot = o.output_dir.join(&name);
                        let worker_report = o.output_dir.join(format!(
                            ".case-{ri}-{}x{}.json",
                            viewport.width, viewport.height
                        ));
                        // A rerun must never accept an old result or stale screenshot.
                        for file in [&screenshot, &worker_report] {
                            if file.is_file() {
                                fs::remove_file(file)?;
                            }
                        }
                        let mut case = ReviewCase {
                            route: route.clone(),
                            requested_url: url.to_string(),
                            viewport: *viewport,
                            status: "incomplete".into(),
                            result: BrowserReview::default(),
                        };
                        if cancelled() {
                            case.result.failure = Some("cancelled: rerun review when ready".into());
                            report.cases.push(case);
                            continue;
                        }
                        server.check_running()?;
                        let mut cmd = Command::new(std::env::current_exe()?);
                        cmd.arg("test-visual-case")
                            .arg("--url")
                            .arg(url.as_str())
                            .arg("--mount-url")
                            .arg(base.as_str())
                            .arg("--target")
                            .arg(o.target.as_str())
                            .arg("--width")
                            .arg(viewport.width.to_string())
                            .arg("--height")
                            .arg(viewport.height.to_string())
                            .arg("--timeout-ms")
                            .arg(o.case_timeout.as_millis().to_string())
                            .arg("--screenshot")
                            .arg(&screenshot)
                            .arg("--report")
                            .arg(&worker_report);
                        match run_captured(
                            &mut cmd,
                            "review browser case",
                            o.case_timeout,
                            &mut *cancelled,
                        ) {
                            Ok(_) => {
                                case.result = serde_json::from_slice(
                                    &fs::read(&worker_report).context("worker report missing")?,
                                )?;
                                if case.result.screenshot.is_some() {
                                    if !screenshot.is_file() {
                                        case.result.failure =
                                            Some("capture: claimed image is missing".into());
                                        case.result.screenshot = None;
                                    } else {
                                        case.result.screenshot = Some(PathBuf::from(&name));
                                    }
                                }
                                case.status = case_status(&case.result).into();
                            }
                            Err(error) => {
                                if error.downcast_ref::<CleanupError>().is_some() {
                                    report.owned_resources_released = false;
                                    return Err(error);
                                }
                                case.result.failure=Some(format!("browser_worker: {}; retry after repairing browser/runtime or increase --case-timeout-seconds",bounded(&format!("{error:#}"))));
                            }
                        }
                        if worker_report.is_file() {
                            fs::remove_file(worker_report)?;
                        }
                        report.cases.push(case);
                        // Persist completed cases before continuing the bounded matrix.
                        write_report(&report)?;
                    }
                }
                server.check_running()?;
                Ok(())
            },
        );
        if session.interrupted()
            && preview_result
                .as_ref()
                .err()
                .is_none_or(|e| e.downcast_ref::<CleanupError>().is_none())
        {
            report.limitations.push(
                "Review cancelled; coverage is incomplete. Rerun the same command when ready."
                    .into(),
            );
            report.status = "incomplete".into();
            report.exit_code = 2;
            return Ok(());
        }
        preview_result?;
        let (status, code) = overall(&report.cases, o.strict);
        report.status = status.into();
        report.exit_code = code;
        Ok(())
    })();
    if let Err(error) = result {
        report.status = "execution_failed".into();
        report.exit_code = 1;
        report.owned_resources_released &= error.downcast_ref::<CleanupError>().is_none();
        report.failure=Some(Failure {stage:"review_execution".into(),evidence:bounded(&format!("{error:#}")),recovery:if report.owned_resources_released { "Correct the reported build/configuration/asset error and rerun the same review command. For bind failures use --port 0. Owned resources are stopped; never kill unrelated listeners by port." } else { "Owned worker cleanup could not be confirmed. Inspect the reported OS failure and only this session's process tree before retrying; do not terminate unrelated listeners by port." }.into()});
    }
    if let Err(error) = write_report(&report) {
        report.exit_code = 1;
        report.status = "execution_failed".into();
        report.failure = Some(Failure {
            stage: "report_write".into(),
            evidence: bounded(&format!("{error:#}")),
            recovery: "Choose a writable --output-dir and rerun.".into(),
        });
    }
    if o.json {
        serde_json::to_writer(io::stdout().lock(), &report)?;
        println!();
    } else {
        eprintln!(
            "Review {}: {} case(s), report {}",
            report.status,
            report.cases.len(),
            report.output_dir.join("review.json").display()
        );
        for case in &report.cases {
            if case.status != "clean_supported_checks" {
                eprintln!(
                    "{} {}x{}: {} ({} finding(s), {} observation(s))",
                    case.route,
                    case.viewport.width,
                    case.viewport.height,
                    case.status,
                    case.result.findings.len(),
                    case.result.observations.len()
                );
            }
        }
        for case in &report.cases {
            if let Some(failure) = &case.result.failure {
                eprintln!("{}: {}", case.route, failure);
            }
            for finding in case.result.findings.iter().take(8) {
                eprintln!(
                    "{} {}: {} — {}",
                    finding.severity,
                    finding.kind,
                    finding.id.as_deref().unwrap_or("unknown node"),
                    finding.evidence
                );
            }
            for observation in case
                .result
                .observations
                .iter()
                .filter(|o| !o.optional)
                .take(8)
            {
                eprintln!(
                    "{} {}: {}",
                    observation.kind,
                    observation.url.as_deref().unwrap_or("runtime"),
                    observation.evidence
                );
            }
        }
        if let Some(f) = &report.failure {
            eprintln!("{}: {}\n{}", f.stage, f.evidence, f.recovery);
        }
        eprintln!(
            "Inspect screenshot files and partial-check limitations before accepting the website."
        );
    }
    if report.exit_code != 0 {
        return Err(ReviewExit(report.exit_code).into());
    }
    Ok(())
}
fn write_report(report: &ReviewReport) -> Result<()> {
    let path = report.output_dir.join("review.json");
    let mut file = fs::File::create(&path)?;
    serde_json::to_writer_pretty(&mut file, report)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn bounded(s: &str) -> String {
    s.chars().take(4096).collect()
}
pub fn normalize_route(route: &str) -> Result<String> {
    if !route.starts_with('/')
        || route.contains("//")
        || route.split('/').any(|s| s == "." || s == "..")
        || !route
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.~".contains(&b))
    {
        bail!("invalid route path; use app-relative / or /about/ without query, fragment, percent escapes or traversal");
    }
    Ok(route.to_owned())
}
fn route_url(base: &Url, route: &str) -> Result<Url> {
    let route = normalize_route(route)?;
    let url = base.join(route.trim_start_matches('/'))?;
    if url.origin() != base.origin() || !url.path().starts_with(base.path()) {
        bail!("route escaped preview mount");
    }
    Ok(url)
}
fn artifact_name(route_index: usize, v: Viewport) -> String {
    format!("route-{route_index:03}-{}x{}.png", v.width, v.height)
}
fn case_status(r: &BrowserReview) -> &'static str {
    if r.failure.is_some() || !r.ready || r.screenshot.is_none() {
        "incomplete"
    } else if r.observations.iter().any(|o| !o.optional)
        || r.findings.iter().any(|f| f.severity == "error")
    {
        "defects_detected"
    } else if !r.findings.is_empty() {
        "warning_candidates"
    } else {
        "clean_supported_checks"
    }
}
fn overall(cases: &[ReviewCase], strict: bool) -> (&'static str, i32) {
    if cases.is_empty() || cases.iter().any(|c| c.status == "incomplete") {
        ("incomplete", 2)
    } else if cases.iter().any(|c| c.status == "defects_detected") {
        ("defects_detected", 3)
    } else if cases.iter().any(|c| c.status == "warning_candidates") {
        ("warning_candidates", if strict { 3 } else { 0 })
    } else {
        ("clean_supported_checks", 0)
    }
}
pub struct ReviewWorkerOptions {
    pub url: String,
    pub mount: String,
    pub target: Target,
    pub viewport: Viewport,
    pub timeout_ms: u64,
    pub screenshot: PathBuf,
    pub report: PathBuf,
}
pub fn worker(o: ReviewWorkerOptions) -> Result<()> {
    let ReviewWorkerOptions {
        url,
        mount,
        target,
        viewport,
        timeout_ms,
        screenshot,
        report,
    } = o;
    in_owned_process_tree(|| {
        let mut options = BrowserTestOptions::new(url).screenshot(screenshot);
        if target == Target::Web {
            options = options.fission_canvas();
        }
        options.viewport_width = viewport.width;
        options.viewport_height = viewport.height;
        options.timeout_ms = timeout_ms.saturating_sub(1500).max(100);
        let result = review_case(options, &mount)?;
        fs::write(report, serde_json::to_vec(&result)?)?;
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_remain_in_mount_and_reject_url_traversal() {
        let base = Url::parse("http://localhost:123/repository-name/").unwrap();
        assert_eq!(
            route_url(&base, "/about/").unwrap().as_str(),
            "http://localhost:123/repository-name/about/"
        );
        for route in [
            "//evil.test/",
            "/../x",
            "/%2e%2e/x",
            "https://evil.test/",
            "/x?y",
            "/x#y",
            "/./x",
            "/x\\y",
        ] {
            assert!(route_url(&base, route).is_err());
        }
    }
    #[test]
    fn explicit_viewports_are_bounded_and_names_are_deterministic() {
        let v: Viewport = "390x900".parse().unwrap();
        assert_eq!(artifact_name(1, v), "route-001-390x900.png");
        for s in ["0x900", "390", "900x5000", "390x900x2"] {
            assert!(s.parse::<Viewport>().is_err());
        }
    }
    #[test]
    fn failure_or_defect_cannot_be_clean() {
        let mut r = BrowserReview::default();
        assert_eq!(case_status(&r), "incomplete");
        r.ready = true;
        r.screenshot = Some("real.png".into());
        assert_eq!(case_status(&r), "clean_supported_checks");
        r.failure = Some("timeout".into());
        assert_eq!(case_status(&r), "incomplete");
        assert_eq!(overall(&[], false), ("incomplete", 2));
    }
    fn complete_case() -> ReviewCase {
        ReviewCase {
            route: "/".into(),
            requested_url: "http://localhost/".into(),
            viewport: Viewport {
                width: 390,
                height: 900,
            },
            status: "clean_supported_checks".into(),
            result: BrowserReview {
                ready: true,
                screenshot: Some("real.png".into()),
                ..Default::default()
            },
        }
    }
    #[test]
    fn warning_candidates_only_fail_an_explicit_strict_policy() {
        let mut case = complete_case();
        case.result
            .findings
            .push(fission_test_driver::browser::review::Finding {
                kind: "horizontal_bounds_candidate".into(),
                severity: "warning".into(),
                id: Some("content".into()),
                bounds: None,
                evidence: "Inspect image; clipping intent is unknown".into(),
            });
        case.status = case_status(&case.result).into();
        assert_eq!(case.status, "warning_candidates");
        assert_eq!(
            overall(std::slice::from_ref(&case), false),
            ("warning_candidates", 0)
        );
        assert_eq!(
            overall(std::slice::from_ref(&case), true),
            ("warning_candidates", 3)
        );
        case.result.findings[0].severity = "error".into();
        case.status = case_status(&case.result).into();
        assert_eq!(
            overall(std::slice::from_ref(&case), false),
            ("defects_detected", 3)
        );
        case.result.failure = Some("observation_limit".into());
        case.status = case_status(&case.result).into();
        assert_eq!(
            overall(std::slice::from_ref(&case), false),
            ("incomplete", 2)
        );
        assert_eq!(overall(&[case], true), ("incomplete", 2));
    }
    #[test]
    fn confirmed_resource_errors_block_even_without_strict_warnings() {
        let mut case = complete_case();
        case.result
            .observations
            .push(fission_test_driver::browser::review::Observation {
                kind: "resource_failure".into(),
                url: Some("http://localhost/bootstrap.mjs".into()),
                optional: false,
                evidence: "HTTP 404 Script".into(),
            });
        case.status = case_status(&case.result).into();
        assert_eq!(overall(&[case], false), ("defects_detected", 3));
    }
    #[test]
    fn lexical_paths_do_not_require_filesystem_resolution() {
        let path = PathBuf::from("missing-mapped-project/../review-output");
        assert_eq!(
            absolute_path(&path).unwrap(),
            std::env::current_dir().unwrap().join(path)
        );
        let absolute = std::env::temp_dir()
            .join("fission-uncreated-mapped-path")
            .join("../output");
        assert_eq!(absolute_path(&absolute).unwrap(), absolute);
    }
    #[cfg(windows)]
    #[test]
    fn windows_mapped_and_unc_paths_keep_their_lexical_spelling() {
        for path in [r"Z:\mapped\app\..\review", r"\\server\webdav\app\review"] {
            let path = PathBuf::from(path);
            assert!(path.is_absolute());
            assert_eq!(absolute_path(&path).unwrap(), path);
        }
    }
}
