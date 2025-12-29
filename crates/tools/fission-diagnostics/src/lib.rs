use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// --------- Public Types ---------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DiagLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl DiagLevel {
    pub fn allows(self, level: DiagLevel) -> bool {
        use DiagLevel::*;
        let a = match self {
            Error => 0,
            Warn => 1,
            Info => 2,
            Debug => 3,
            Trace => 4,
        };
        let b = match level {
            Error => 0,
            Warn => 1,
            Info => 2,
            Debug => 3,
            Trace => 4,
        };
        b <= a || b == a || b >= 0 && a >= 0 && b <= a
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum DiagCategory {
    Frame,
    Diff,
    Layout,
    Paint,
    Raster,
    Input,
    Semantics,
    Animation,
    Media,
    Invariants,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagEvent {
    pub schema_version: u16, // v1 = 1
    pub timestamp_ns: u64,
    pub frame_no: u64,
    pub category: DiagCategory,
    pub level: DiagLevel,
    #[serde(flatten)]
    pub event: DiagEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DiagEventKind {
    FrameStart { root: Option<u128> },
    FrameEnd { stats: FrameStats },

    DiffSummary {
        nodes_total: u32,
        nodes_created: u32,
        nodes_removed: u32,
        nodes_changed: u32,
        dirty_layout: u32,
        dirty_paint: u32,
    },

    LayoutSummary {
        nodes: u32,
        dirty_count: u32,
        full_rebuild: bool,
    },

    PaintSummary {
        segments_reused: u32,
        segments_regenerated: u32,
        paint_ops_total: u32,
    },
    PaintNode {
        node: u128,
        note: Option<String>,
    },
    PaintNodeRect {
        node: u128,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        note: Option<String>,
    },

    RasterSummary {
        cache_hits: u32,
        cache_misses: u32,
        tiles_rasterized: u32,
    },

    AnimationSummary {
        active_count: u32,
        started: u32,
        replaced: u32,
        ended: u32,
    },

    MediaSummary {
        video_nodes: u32,
        audio_nodes: u32,
        embeds_total: u32,
    },

    // Overlay/Portal + Anchor diagnostics (layout investigation helpers)
    PortalsComposed { portal_count: u32 },
    AnchorPlacement {
        widget: u128,
        node: u128,
        rect_x: f32,
        rect_y: f32,
        rect_w: f32,
        rect_h: f32,
        place_left: f32,
        place_top: f32,
        note: Option<String>,
    },

    InvariantViolation {
        kind: String,
        node: Option<u128>,
        details: String,
        dump_ref: Option<String>,
    },

    InputEvent {
        kind: String,
        target: Option<u128>,
        position: Option<(f32, f32)>,
    },

    MediaEvent {
        kind: String,
        id: Option<u128>,
        duration_ms: Option<u64>,
        position_ms: Option<u64>,
    },

    // Text input auto-scroll diagnostics
    TextInputAutoScroll {
        scroll_id: u128,
        text_id: u128,
        text_len: u32,
        measured_w: f32,
        line_h: f32,
        viewport_x: f32,
        viewport_w: f32,
        content_w: f32,
        caret_abs_x: f32,
        offset_before: f32,
        offset_after: f32,
    },

    // General scrolling diagnostics
    ScrollExtent {
        node: u128,
        viewport_w: f32,
        viewport_h: f32,
        content_w: f32,
        content_h: f32,
        note: Option<String>,
    },
    ScrollUpdate {
        node: u128,
        axis: String,
        point_x: f32,
        point_y: f32,
        delta: f32,
        old_offset: f32,
        new_offset: f32,
        max_offset: f32,
        viewport_w: f32,
        viewport_h: f32,
        content_w: f32,
        content_h: f32,
    },
    ScrollPaintTranslate {
        node: u128,
        axis: String,
        offset: f32,
        translate_x: f32,
        translate_y: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrameStats {
    pub dirty_nodes: u32,
    pub layout_updates: u32,
    pub paint_misses: u32,
    pub paint_hits: u32,
    pub video_surfaces: u32,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsConfig {
    pub enabled_categories: BTreeSet<DiagCategory>,
    pub min_level: DiagLevel,
    pub sink: DiagSink,
    pub sampling: f32,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            enabled_categories: BTreeSet::new(),
            min_level: DiagLevel::Error,
            sink: DiagSink::Stdout,
            sampling: 1.0,
        }
    }
}

// --------- Sinks ---------

#[derive(Debug, Clone)]
pub enum DiagSink {
    Stdout,
    File(PathBuf),
    RingBuffer(usize),
    Disabled,
}

trait SinkImpl: Send + Sync {
    fn write(&self, event: &DiagEvent);
}

struct StdoutSinkImpl;
impl SinkImpl for StdoutSinkImpl {
    fn write(&self, event: &DiagEvent) {
        // JSONL for stable tooling integration
        let _ = serde_json::to_string(event)
            .map(|line| println!("{}", line));
    }
}

struct FileSinkImpl {
    file: RwLock<File>,
}
impl SinkImpl for FileSinkImpl {
    fn write(&self, event: &DiagEvent) {
        if let Ok(s) = serde_json::to_string(event) {
            let mut f = self.file.write();
            let _ = f.write_all(s.as_bytes());
            let _ = f.write_all(b"\n");
        }
    }
}

struct RingBufferSinkImpl {
    // very simple ring buffer of JSON strings for now
    buf: RwLock<Vec<String>>,
    cap: usize,
}
impl SinkImpl for RingBufferSinkImpl {
    fn write(&self, event: &DiagEvent) {
        if let Ok(s) = serde_json::to_string(event) {
            let mut w = self.buf.write();
            if w.len() >= self.cap { w.remove(0); }
            w.push(s);
        }
    }
}

// --------- Global Diagnostics ---------

struct DiagnosticsInner {
    config: DiagnosticsConfig,
    sink_impl: Box<dyn SinkImpl>,
    frame_no: AtomicU64,
    timestamp_ns: AtomicU64,
}

impl DiagnosticsInner {
    fn should_emit(&self, cat: &DiagCategory, level: DiagLevel) -> bool {
        if matches!(self.config.sink, DiagSink::Disabled) { return false; }
        if !self.config.enabled_categories.contains(cat) { return false; }
        self.config.min_level.allows(level)
    }
}

static DIAGNOSTICS: OnceCell<RwLock<DiagnosticsInner>> = OnceCell::new();

pub fn init_from_env() {
    // Categories
    let cats = std::env::var("FISSION_DIAG").unwrap_or_default();
    let enabled_categories: BTreeSet<DiagCategory> = cats
        .split(',')
        .filter_map(|s| match s.trim().to_lowercase().as_str() {
            "frame" => Some(DiagCategory::Frame),
            "diff" => Some(DiagCategory::Diff),
            "layout" => Some(DiagCategory::Layout),
            "paint" => Some(DiagCategory::Paint),
            "raster" => Some(DiagCategory::Raster),
            "input" => Some(DiagCategory::Input),
            "semantics" => Some(DiagCategory::Semantics),
            "animation" => Some(DiagCategory::Animation),
            "media" => Some(DiagCategory::Media),
            "invariants" => Some(DiagCategory::Invariants),
            "test" => Some(DiagCategory::Test),
            "*" => None, // handled below
            _ => None,
        })
        .collect();

    // Level
    let min_level = match std::env::var("FISSION_DIAG_LEVEL").unwrap_or_default().to_lowercase().as_str() {
        "error" => DiagLevel::Error,
        "warn" => DiagLevel::Warn,
        "info" => DiagLevel::Info,
        "debug" => DiagLevel::Debug,
        "trace" => DiagLevel::Trace,
        _ => DiagLevel::Warn,
    };

    // Sink
    let sink_env = std::env::var("FISSION_DIAG_SINK").unwrap_or_default();
    let sink = if sink_env.starts_with("file:") {
        DiagSink::File(PathBuf::from(sink_env.trim_start_matches("file:")))
    } else if sink_env.starts_with("ipc:") {
        // Not implemented v1; fallback to stdout
        DiagSink::Stdout
    } else if sink_env == "stdout" || sink_env.is_empty() {
        DiagSink::Stdout
    } else {
        DiagSink::Disabled
    };

    let sampling = std::env::var("FISSION_DIAG_SAMPLING")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);

    let mut cfg = DiagnosticsConfig {
        enabled_categories,
        min_level,
        sink,
        sampling,
    };

    // Handle wildcard * for categories (enable all)
    if cats.split(',').any(|s| s.trim() == "*") {
        cfg.enabled_categories = [
            DiagCategory::Frame,
            DiagCategory::Diff,
            DiagCategory::Layout,
            DiagCategory::Paint,
            DiagCategory::Raster,
            DiagCategory::Input,
            DiagCategory::Semantics,
            DiagCategory::Animation,
            DiagCategory::Media,
            DiagCategory::Invariants,
            DiagCategory::Test,
        ]
        .into_iter()
        .collect();
    }

    init(cfg);
}

pub fn init(config: DiagnosticsConfig) {
    let sink_impl: Box<dyn SinkImpl> = match &config.sink {
        DiagSink::Stdout => Box::new(StdoutSinkImpl),
        DiagSink::File(path) => {
            let file = OpenOptions::new().create(true).append(true).open(path).unwrap();
            Box::new(FileSinkImpl { file: RwLock::new(file) })
        }
        DiagSink::RingBuffer(cap) => Box::new(RingBufferSinkImpl { buf: RwLock::new(Vec::with_capacity(*cap)), cap: *cap }),
        DiagSink::Disabled => Box::new(StdoutSinkImpl), // won't be used
    };

    let inner = DiagnosticsInner {
        config,
        sink_impl,
        frame_no: AtomicU64::new(0),
        timestamp_ns: AtomicU64::new(0),
    };
    let _ = DIAGNOSTICS.set(RwLock::new(inner));
}

fn with_diag<T>(f: impl FnOnce(&DiagnosticsInner) -> T) -> Option<T> {
    DIAGNOSTICS.get().map(|cell| {
        let guard = cell.read();
        f(&*guard)
    })
}

fn with_diag_mut<T>(f: impl FnOnce(&mut DiagnosticsInner) -> T) -> Option<T> {
    DIAGNOSTICS.get().map(|cell| {
        let mut guard = cell.write();
        f(&mut *guard)
    })
}

pub fn begin_frame(root: Option<u128>) {
    let _ = with_diag_mut(|d| {
        let ts = d.timestamp_ns.fetch_add(16666666, Ordering::Relaxed) + 1; // ~60fps increment
        let fno = d.frame_no.fetch_add(1, Ordering::Relaxed) + 1;
        let ev = DiagEvent {
            schema_version: 1,
            timestamp_ns: ts,
            frame_no: fno,
            category: DiagCategory::Frame,
            level: DiagLevel::Debug,
            event: DiagEventKind::FrameStart { root },
        };
        if d.should_emit(&ev.category, ev.level) {
            d.sink_impl.write(&ev);
        }
    });
}

pub fn end_frame(stats: FrameStats) {
    let _ = with_diag_mut(|d| {
        let ts = d.timestamp_ns.fetch_add(1, Ordering::Relaxed) + 1;
        let fno = d.frame_no.load(Ordering::Relaxed);
        let ev = DiagEvent {
            schema_version: 1,
            timestamp_ns: ts,
            frame_no: fno,
            category: DiagCategory::Frame,
            level: DiagLevel::Debug,
            event: DiagEventKind::FrameEnd { stats },
        };
        if d.should_emit(&ev.category, ev.level) {
            d.sink_impl.write(&ev);
        }
    });
}

pub fn emit(category: DiagCategory, level: DiagLevel, event: DiagEventKind) {
    let _ = with_diag_mut(|d| {
        if !d.should_emit(&category, level) { return; }
        let ts = d.timestamp_ns.fetch_add(1, Ordering::Relaxed) + 1;
        let fno = d.frame_no.load(Ordering::Relaxed);
        let ev = DiagEvent {
            schema_version: 1,
            timestamp_ns: ts,
            frame_no: fno,
            category,
            level,
            event,
        };
        d.sink_impl.write(&ev);
    });
}

// Helpers for common categories
pub mod prelude {
    pub use super::{begin_frame, end_frame, emit, DiagCategory, DiagEventKind, DiagLevel, FrameStats, init_from_env};
}

// --------- Snapshot Provider (v1 minimal) ---------

#[derive(Debug, Clone, Copy)]
pub enum SnapshotKind { Layout }

#[derive(Debug, Clone)]
pub struct SnapshotBlob {
    pub kind: SnapshotKind,
    pub json: String,
}

pub trait SnapshotProvider {
    fn snapshot(&self, kind: SnapshotKind) -> Option<SnapshotBlob>;
}
