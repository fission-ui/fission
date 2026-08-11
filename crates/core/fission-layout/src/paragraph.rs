//! Backend-neutral paragraph input, geometry, and query contracts.
//!
//! A paragraph engine produces one self-contained result for a normalized
//! description. Layout, painting, editing, IME, and accessibility consume that
//! same result so text geometry cannot drift between subsystems.

use fission_ir::op::{TextDirection, TextParagraphStyle, TextStyle};
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::{LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};

/// A byte offset at a UTF-8 code-point boundary.
///
/// Fission text indices are always offsets into the UTF-8 source string. They
/// are never UTF-16 code-unit, Unicode scalar, grapheme, or glyph indices.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct Utf8Index(usize);

impl Utf8Index {
    /// Creates an index from a UTF-8 byte offset.
    pub const fn new(byte_offset: usize) -> Self {
        Self(byte_offset)
    }

    /// Returns the UTF-8 byte offset.
    pub const fn byte_offset(self) -> usize {
        self.0
    }

    /// Returns whether this index is a valid boundary in text.
    pub fn is_boundary_in(self, text: &str) -> bool {
        self.0 <= text.len() && text.is_char_boundary(self.0)
    }
}

impl From<usize> for Utf8Index {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl From<Utf8Index> for usize {
    fn from(value: Utf8Index) -> Self {
        value.byte_offset()
    }
}

/// A half-open UTF-8 byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Utf8Range {
    start: Utf8Index,
    end: Utf8Index,
}

impl<'de> Deserialize<'de> for Utf8Range {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedRange {
            start: Utf8Index,
            end: Utf8Index,
        }

        let range = SerializedRange::deserialize(deserializer)?;
        Self::new(range.start, range.end).ok_or_else(|| {
            de::Error::custom(format_args!(
                "UTF-8 range start {} exceeds end {}",
                range.start.byte_offset(),
                range.end.byte_offset()
            ))
        })
    }
}

impl Utf8Range {
    /// Creates a range when start is no later than end.
    pub const fn new(start: Utf8Index, end: Utf8Index) -> Option<Self> {
        if start.0 <= end.0 {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Creates an empty range at index.
    pub const fn empty(index: Utf8Index) -> Self {
        Self {
            start: index,
            end: index,
        }
    }

    /// Creates a range from byte offsets, returning a descriptive error for an
    /// inverted range.
    pub fn from_byte_offsets(start: usize, end: usize) -> Result<Self, ParagraphError> {
        Self::new(Utf8Index::new(start), Utf8Index::new(end)).ok_or_else(|| {
            ParagraphError::invalid_description(
                "range",
                format!("UTF-8 range start {start} exceeds end {end}"),
            )
        })
    }

    pub const fn start(self) -> Utf8Index {
        self.start
    }

    pub const fn end(self) -> Utf8Index {
        self.end
    }

    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }

    pub const fn len(self) -> usize {
        self.end.0 - self.start.0
    }

    pub const fn contains(self, index: Utf8Index) -> bool {
        self.start.0 <= index.0 && index.0 < self.end.0
    }

    pub const fn contains_range(self, other: Self) -> bool {
        self.start.0 <= other.start.0 && other.end.0 <= self.end.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.start.0 < other.end.0 && other.start.0 < self.end.0
    }

    /// Validates both offsets as UTF-8 boundaries in text.
    pub fn validate_in(self, text: &str, field: &'static str) -> Result<(), ParagraphError> {
        if !self.start.is_boundary_in(text) {
            return Err(ParagraphError::invalid_description(
                field,
                format!(
                    "start offset {} is not a UTF-8 boundary for {} bytes of text",
                    self.start.0,
                    text.len()
                ),
            ));
        }
        if !self.end.is_boundary_in(text) {
            return Err(ParagraphError::invalid_description(
                field,
                format!(
                    "end offset {} is not a UTF-8 boundary for {} bytes of text",
                    self.end.0,
                    text.len()
                ),
            ));
        }
        Ok(())
    }
}

/// A variable-font axis value. tag uses the big-endian OpenType four-byte tag.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphFontVariation {
    pub tag: u32,
    pub value: f32,
}

/// An OpenType feature value. tag uses the big-endian OpenType four-byte tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParagraphFontFeature {
    pub tag: u32,
    pub value: u32,
}

/// One normalized style range over the paragraph's shared UTF-8 string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphStyleRun {
    pub range: Utf8Range,
    pub style: TextStyle,
    /// Relative font width, where 1.0 is normal width.
    pub font_width: f32,
    pub word_spacing: LayoutUnit,
    pub variations: Vec<ParagraphFontVariation>,
    pub features: Vec<ParagraphFontFeature>,
}

impl ParagraphStyleRun {
    pub fn new(range: Utf8Range, style: TextStyle) -> Self {
        Self {
            range,
            style,
            font_width: 1.0,
            word_spacing: 0.0,
            variations: Vec::new(),
            features: Vec::new(),
        }
    }
}

/// An inline object placeholder in the input paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphInlineObject {
    pub id: u64,
    pub range: Utf8Range,
    pub size: LayoutSize,
    /// Distance from the top of the box to the alphabetic baseline.
    pub baseline: LayoutUnit,
}

/// Transient IME preedit state expressed in source UTF-8 coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphPreedit {
    pub range: Utf8Range,
    /// The IME selection, also in source coordinates and contained by range.
    pub selection: Utf8Range,
}

/// A normalized, backend-neutral paragraph request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphDescription {
    pub text: String,
    pub style_runs: Vec<ParagraphStyleRun>,
    pub paragraph_style: TextParagraphStyle,
    /// None means unbounded width.
    pub width_constraint: Option<LayoutUnit>,
    pub wrap: bool,
    pub locale: Option<String>,
    pub inline_objects: Vec<ParagraphInlineObject>,
    pub selection: Option<Utf8Range>,
    pub preedit: Option<ParagraphPreedit>,
    /// Changes whenever the active Fission font catalogue changes.
    pub font_catalog_generation: u64,
    /// Ordered fallback families after run-local families.
    pub fallback_families: Vec<String>,
}

impl ParagraphDescription {
    pub fn new(
        text: impl Into<String>,
        style_runs: Vec<ParagraphStyleRun>,
        paragraph_style: TextParagraphStyle,
        width_constraint: Option<LayoutUnit>,
    ) -> Self {
        Self {
            text: text.into(),
            style_runs,
            paragraph_style,
            width_constraint,
            wrap: true,
            locale: None,
            inline_objects: Vec::new(),
            selection: None,
            preedit: None,
            font_catalog_generation: 0,
            fallback_families: Vec::new(),
        }
    }

    /// Validates normalized runs, UTF-8 boundaries, and finite layout inputs.
    pub fn validate(&self) -> Result<(), ParagraphError> {
        if self
            .width_constraint
            .is_some_and(|width| !width.is_finite() || width < 0.0)
        {
            return Err(ParagraphError::invalid_description(
                "width_constraint",
                "width must be finite and non-negative",
            ));
        }
        if self
            .paragraph_style
            .strut_line_height
            .is_some_and(|height| !height.is_finite() || height <= 0.0)
        {
            return Err(ParagraphError::invalid_description(
                "paragraph_style.strut_line_height",
                "strut line height must be finite and positive",
            ));
        }

        let mut covered_until = Utf8Index::new(0);
        for (index, run) in self.style_runs.iter().enumerate() {
            run.range.validate_in(&self.text, "style_runs.range")?;
            if !self.text.is_empty() && run.range.is_empty() {
                return Err(ParagraphError::invalid_description(
                    "style_runs",
                    format!("style run {index} is empty"),
                ));
            }
            if run.range.start() != covered_until {
                return Err(ParagraphError::invalid_description(
                    "style_runs",
                    format!(
                        "style run {index} starts at {}, expected {}",
                        run.range.start().byte_offset(),
                        covered_until.byte_offset()
                    ),
                ));
            }
            validate_style_run(run, index)?;
            covered_until = run.range.end();
        }
        if self.text.is_empty() {
            if self.style_runs.len() > 1
                || self
                    .style_runs
                    .first()
                    .is_some_and(|run| !run.range.is_empty())
            {
                return Err(ParagraphError::invalid_description(
                    "style_runs",
                    "empty text accepts at most one empty style run",
                ));
            }
        } else if covered_until.byte_offset() != self.text.len() {
            return Err(ParagraphError::invalid_description(
                "style_runs",
                format!(
                    "style runs cover {} of {} UTF-8 bytes",
                    covered_until.byte_offset(),
                    self.text.len()
                ),
            ));
        }

        if let Some(selection) = self.selection {
            selection.validate_in(&self.text, "selection")?;
        }
        if let Some(preedit) = self.preedit {
            preedit.range.validate_in(&self.text, "preedit.range")?;
            preedit
                .selection
                .validate_in(&self.text, "preedit.selection")?;
            if !preedit.range.contains_range(preedit.selection) {
                return Err(ParagraphError::invalid_description(
                    "preedit.selection",
                    "preedit selection must be contained by the preedit range",
                ));
            }
        }

        let mut inline_ids = HashSet::with_capacity(self.inline_objects.len());
        let mut previous_inline_end = Utf8Index::new(0);
        for (index, inline) in self.inline_objects.iter().enumerate() {
            inline
                .range
                .validate_in(&self.text, "inline_objects.range")?;
            if inline.range.is_empty() {
                return Err(ParagraphError::invalid_description(
                    "inline_objects.range",
                    "inline object ranges cannot be empty",
                ));
            }
            if !valid_size(inline.size) || !inline.baseline.is_finite() {
                return Err(ParagraphError::invalid_description(
                    "inline_objects",
                    "inline size and baseline must be finite and size must be non-negative",
                ));
            }
            if index > 0 && inline.range.start() < previous_inline_end {
                return Err(ParagraphError::invalid_description(
                    "inline_objects.range",
                    format!(
                        "inline object {index} overlaps an earlier object or is not in source order"
                    ),
                ));
            }
            if !inline_ids.insert(inline.id) {
                return Err(ParagraphError::invalid_description(
                    "inline_objects.id",
                    format!("duplicate inline object id {}", inline.id),
                ));
            }
            previous_inline_end = inline.range.end();
        }

        Ok(())
    }

    /// Returns capabilities implied by this particular description.
    pub fn required_capabilities(&self) -> ParagraphCapabilities {
        let mut required = ParagraphCapabilities::NONE;
        if self.paragraph_style.text_direction != TextDirection::Ltr {
            required = required.with(ParagraphCapability::BidirectionalText);
        }
        if self.style_runs.iter().any(|run| !run.variations.is_empty()) {
            required = required.with(ParagraphCapability::VariableFonts);
        }
        if self.style_runs.iter().any(|run| !run.features.is_empty()) {
            required = required.with(ParagraphCapability::FontFeatures);
        }
        if !self.inline_objects.is_empty() {
            required = required.with(ParagraphCapability::InlineObjects);
        }
        if self.selection.is_some() {
            required = required
                .with(ParagraphCapability::ClusterMapping)
                .with(ParagraphCapability::SelectionGeometry);
        }
        if self.preedit.is_some() {
            required = required
                .with(ParagraphCapability::ClusterMapping)
                .with(ParagraphCapability::CaretGeometry)
                .with(ParagraphCapability::SelectionGeometry);
        }
        required
    }
}

fn validate_style_run(run: &ParagraphStyleRun, index: usize) -> Result<(), ParagraphError> {
    if !run.style.font_size.is_finite() || run.style.font_size <= 0.0 {
        return Err(ParagraphError::invalid_description(
            "style_runs.style.font_size",
            format!("style run {index} font size must be finite and positive"),
        ));
    }
    if !run.font_width.is_finite() || run.font_width <= 0.0 {
        return Err(ParagraphError::invalid_description(
            "style_runs.font_width",
            format!("style run {index} font width must be finite and positive"),
        ));
    }
    if !run.word_spacing.is_finite() || !run.style.letter_spacing.is_finite() {
        return Err(ParagraphError::invalid_description(
            "style_runs.spacing",
            format!("style run {index} spacing must be finite"),
        ));
    }
    if run
        .style
        .line_height
        .is_some_and(|height| !height.is_finite() || height <= 0.0)
    {
        return Err(ParagraphError::invalid_description(
            "style_runs.style.line_height",
            format!("style run {index} line height must be finite and positive"),
        ));
    }
    if run
        .variations
        .iter()
        .any(|variation| !variation.value.is_finite())
    {
        return Err(ParagraphError::invalid_description(
            "style_runs.variations",
            format!("style run {index} contains a non-finite variation"),
        ));
    }
    Ok(())
}

/// Optional paragraph-engine behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ParagraphCapability {
    BidirectionalText = 0,
    VariableFonts = 1,
    FontFeatures = 2,
    InlineObjects = 3,
    ClusterMapping = 4,
    HitTesting = 5,
    CaretGeometry = 6,
    SelectionGeometry = 7,
    UnresolvedGlyphDiagnostics = 8,
}

impl ParagraphCapability {
    const ALL: [Self; 9] = [
        Self::BidirectionalText,
        Self::VariableFonts,
        Self::FontFeatures,
        Self::InlineObjects,
        Self::ClusterMapping,
        Self::HitTesting,
        Self::CaretGeometry,
        Self::SelectionGeometry,
        Self::UnresolvedGlyphDiagnostics,
    ];

    const fn bit(self) -> u64 {
        1 << self as u8
    }
}

/// A compact, forward-compatible set of paragraph capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct ParagraphCapabilities(u64);

impl ParagraphCapabilities {
    pub const NONE: Self = Self(0);

    pub const fn with(self, capability: ParagraphCapability) -> Self {
        Self(self.0 | capability.bit())
    }

    pub const fn supports(self, capability: ParagraphCapability) -> bool {
        self.0 & capability.bit() != 0
    }

    pub fn require(self, capability: ParagraphCapability) -> Result<(), ParagraphCapabilityError> {
        if self.supports(capability) {
            Ok(())
        } else {
            Err(ParagraphCapabilityError { capability })
        }
    }

    pub fn require_all(self, required: Self) -> Result<(), ParagraphCapabilityError> {
        for capability in ParagraphCapability::ALL {
            if required.supports(capability) && !self.supports(capability) {
                return Err(ParagraphCapabilityError { capability });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphCapabilityError {
    pub capability: ParagraphCapability,
}

impl fmt::Display for ParagraphCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "paragraph engine does not support {:?}",
            self.capability
        )
    }
}

impl Error for ParagraphCapabilityError {}

/// A resolved visual direction. Auto is not valid in measured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParagraphDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParagraphAffinity {
    Upstream,
    Downstream,
}

/// Metrics and logical coverage for one visual line.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphLine {
    pub range: Utf8Range,
    pub rect: LayoutRect,
    pub baseline: LayoutUnit,
    pub ascent: LayoutUnit,
    pub descent: LayoutUnit,
    pub leading: LayoutUnit,
    pub hard_break: bool,
    pub direction: ParagraphDirection,
}

/// Logical and visual data for one shaped cluster.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphCluster {
    pub range: Utf8Range,
    pub rect: LayoutRect,
    pub line_index: usize,
    pub direction: ParagraphDirection,
    pub starts_grapheme: bool,
    pub starts_word: bool,
}

/// A legal caret stop and its rendered rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphCaret {
    pub index: Utf8Index,
    pub affinity: ParagraphAffinity,
    pub rect: LayoutRect,
    pub line_index: usize,
}

/// A selection segment produced in visual order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphSelectionBox {
    pub range: Utf8Range,
    pub rect: LayoutRect,
    pub direction: ParagraphDirection,
}

/// One spatial region used by immutable point-to-text hit testing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphHitRegion {
    pub rect: LayoutRect,
    pub index: Utf8Index,
    pub affinity: ParagraphAffinity,
    pub line_index: usize,
}

/// The result of point-to-text hit testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphHitTest {
    pub index: Utf8Index,
    pub affinity: ParagraphAffinity,
    pub line_index: usize,
    pub is_inside: bool,
}

/// A positioned inline object in measured output.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParagraphInlineBox {
    pub id: u64,
    pub range: Utf8Range,
    pub rect: LayoutRect,
    pub baseline: LayoutUnit,
}

/// A missing-glyph diagnostic tied to source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphUnresolvedGlyph {
    pub range: Utf8Range,
    pub codepoints: Vec<u32>,
}

/// Immutable paragraph geometry shared by all downstream consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphGeometry {
    size: LayoutSize,
    min_intrinsic_width: LayoutUnit,
    max_intrinsic_width: LayoutUnit,
    first_baseline: Option<LayoutUnit>,
    last_baseline: Option<LayoutUnit>,
    lines: Box<[ParagraphLine]>,
    clusters: Box<[ParagraphCluster]>,
    carets: Box<[ParagraphCaret]>,
    hit_regions: Box<[ParagraphHitRegion]>,
    inline_boxes: Box<[ParagraphInlineBox]>,
}

impl ParagraphGeometry {
    pub fn new(size: LayoutSize) -> Self {
        Self {
            size,
            min_intrinsic_width: size.width,
            max_intrinsic_width: size.width,
            first_baseline: None,
            last_baseline: None,
            lines: Box::new([]),
            clusters: Box::new([]),
            carets: Box::new([]),
            hit_regions: Box::new([]),
            inline_boxes: Box::new([]),
        }
    }

    pub fn with_intrinsic_widths(
        mut self,
        min_intrinsic_width: LayoutUnit,
        max_intrinsic_width: LayoutUnit,
    ) -> Self {
        self.min_intrinsic_width = min_intrinsic_width;
        self.max_intrinsic_width = max_intrinsic_width;
        self
    }

    pub fn with_baselines(
        mut self,
        first_baseline: Option<LayoutUnit>,
        last_baseline: Option<LayoutUnit>,
    ) -> Self {
        self.first_baseline = first_baseline;
        self.last_baseline = last_baseline;
        self
    }

    pub fn with_lines(mut self, lines: impl Into<Box<[ParagraphLine]>>) -> Self {
        self.lines = lines.into();
        self
    }

    pub fn with_clusters(mut self, clusters: impl Into<Box<[ParagraphCluster]>>) -> Self {
        self.clusters = clusters.into();
        self
    }

    pub fn with_carets(mut self, carets: impl Into<Box<[ParagraphCaret]>>) -> Self {
        self.carets = carets.into();
        self
    }

    pub fn with_hit_regions(mut self, hit_regions: impl Into<Box<[ParagraphHitRegion]>>) -> Self {
        self.hit_regions = hit_regions.into();
        self
    }

    pub fn with_inline_boxes(mut self, inline_boxes: impl Into<Box<[ParagraphInlineBox]>>) -> Self {
        self.inline_boxes = inline_boxes.into();
        self
    }

    pub const fn size(&self) -> LayoutSize {
        self.size
    }

    pub const fn min_intrinsic_width(&self) -> LayoutUnit {
        self.min_intrinsic_width
    }

    pub const fn max_intrinsic_width(&self) -> LayoutUnit {
        self.max_intrinsic_width
    }

    pub const fn first_baseline(&self) -> Option<LayoutUnit> {
        self.first_baseline
    }

    pub const fn last_baseline(&self) -> Option<LayoutUnit> {
        self.last_baseline
    }

    pub fn lines(&self) -> &[ParagraphLine] {
        &self.lines
    }

    pub fn clusters(&self) -> &[ParagraphCluster] {
        &self.clusters
    }

    pub fn carets(&self) -> &[ParagraphCaret] {
        &self.carets
    }

    pub fn hit_regions(&self) -> &[ParagraphHitRegion] {
        &self.hit_regions
    }

    pub fn inline_boxes(&self) -> &[ParagraphInlineBox] {
        &self.inline_boxes
    }

    fn validate(&self, text: &str) -> Result<(), ParagraphError> {
        if !valid_size(self.size)
            || !valid_scalar(self.min_intrinsic_width)
            || !valid_scalar(self.max_intrinsic_width)
            || self.min_intrinsic_width > self.max_intrinsic_width
        {
            return Err(ParagraphError::invalid_result(
                "dimensions",
                "paragraph dimensions must be finite, non-negative, and ordered",
            ));
        }
        for (name, baseline) in [
            ("first_baseline", self.first_baseline),
            ("last_baseline", self.last_baseline),
        ] {
            if baseline.is_some_and(|value| !value.is_finite()) {
                return Err(ParagraphError::invalid_result(
                    name,
                    "baseline must be finite",
                ));
            }
        }

        let mut previous_end = Utf8Index::new(0);
        for (line_index, line) in self.lines.iter().enumerate() {
            validate_result_range(line.range, text, "lines.range")?;
            if line.range.start() < previous_end {
                return Err(ParagraphError::invalid_result(
                    "lines.range",
                    format!("line {line_index} overlaps an earlier logical line"),
                ));
            }
            if !valid_rect(line.rect)
                || ![line.baseline, line.ascent, line.descent, line.leading]
                    .into_iter()
                    .all(f32::is_finite)
            {
                return Err(ParagraphError::invalid_result(
                    "lines",
                    format!("line {line_index} contains non-finite geometry"),
                ));
            }
            previous_end = line.range.end();
        }

        for cluster in &self.clusters {
            validate_result_range(cluster.range, text, "clusters.range")?;
            validate_line_index(cluster.line_index, self.lines.len(), "clusters.line_index")?;
            if cluster.range.is_empty() {
                return Err(ParagraphError::invalid_result(
                    "clusters.range",
                    "cluster ranges cannot be empty",
                ));
            }
            if !self.lines[cluster.line_index]
                .range
                .contains_range(cluster.range)
            {
                return Err(ParagraphError::invalid_result(
                    "clusters.range",
                    "cluster range must be contained by its referenced line",
                ));
            }
            if !valid_rect(cluster.rect) {
                return Err(ParagraphError::invalid_result(
                    "clusters.rect",
                    "cluster rectangle must be finite and non-negative",
                ));
            }
        }
        for caret in &self.carets {
            validate_result_index(caret.index, text, "carets.index")?;
            validate_line_index(caret.line_index, self.lines.len(), "carets.line_index")?;
            if !line_contains_index(self.lines[caret.line_index].range, caret.index) {
                return Err(ParagraphError::invalid_result(
                    "carets.index",
                    "caret index must be contained by its referenced line",
                ));
            }
            if !valid_rect(caret.rect) {
                return Err(ParagraphError::invalid_result(
                    "carets.rect",
                    "caret rectangle must be finite and non-negative",
                ));
            }
        }
        for hit in &self.hit_regions {
            validate_result_index(hit.index, text, "hit_regions.index")?;
            validate_line_index(hit.line_index, self.lines.len(), "hit_regions.line_index")?;
            if !line_contains_index(self.lines[hit.line_index].range, hit.index) {
                return Err(ParagraphError::invalid_result(
                    "hit_regions.index",
                    "hit-test index must be contained by its referenced line",
                ));
            }
            if !valid_rect(hit.rect) {
                return Err(ParagraphError::invalid_result(
                    "hit_regions.rect",
                    "hit-test rectangle must be finite and non-negative",
                ));
            }
        }
        let mut inline_ids = HashSet::with_capacity(self.inline_boxes.len());
        let mut inline_ranges = HashSet::with_capacity(self.inline_boxes.len());
        for inline in &self.inline_boxes {
            validate_result_range(inline.range, text, "inline_boxes.range")?;
            if inline.range.is_empty() {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes.range",
                    "inline object ranges cannot be empty",
                ));
            }
            if !self
                .lines
                .iter()
                .any(|line| line.range.contains_range(inline.range))
            {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes.range",
                    "inline object range must be contained by a visible line",
                ));
            }
            if !valid_rect(inline.rect) || !inline.baseline.is_finite() {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes",
                    "inline geometry must be finite and non-negative",
                ));
            }
            if !inline_ids.insert(inline.id) {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes.id",
                    format!("duplicate inline object id {}", inline.id),
                ));
            }
            if !inline_ranges.insert(inline.range) {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes.range",
                    format!(
                        "duplicate inline object range {}..{}",
                        inline.range.start().byte_offset(),
                        inline.range.end().byte_offset()
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn line_contains_index(line: Utf8Range, index: Utf8Index) -> bool {
    line.start() <= index && index <= line.end()
}

fn valid_scalar(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

fn valid_size(size: LayoutSize) -> bool {
    valid_scalar(size.width) && valid_scalar(size.height)
}

fn valid_rect(rect: LayoutRect) -> bool {
    rect.origin.x.is_finite() && rect.origin.y.is_finite() && valid_size(rect.size)
}

fn validate_result_index(
    index: Utf8Index,
    text: &str,
    field: &'static str,
) -> Result<(), ParagraphError> {
    if index.is_boundary_in(text) {
        Ok(())
    } else {
        Err(ParagraphError::invalid_result(
            field,
            format!(
                "offset {} is not a UTF-8 boundary for {} bytes of text",
                index.byte_offset(),
                text.len()
            ),
        ))
    }
}

fn validate_result_range(
    range: Utf8Range,
    text: &str,
    field: &'static str,
) -> Result<(), ParagraphError> {
    validate_result_index(range.start(), text, field)?;
    validate_result_index(range.end(), text, field)
}

fn validate_line_index(
    line_index: usize,
    line_count: usize,
    field: &'static str,
) -> Result<(), ParagraphError> {
    if line_index < line_count {
        Ok(())
    } else {
        Err(ParagraphError::invalid_result(
            field,
            format!("line index {line_index} is outside {line_count} lines"),
        ))
    }
}

fn validate_capability_geometry(
    text: &str,
    capabilities: ParagraphCapabilities,
    geometry: &ParagraphGeometry,
    expected_inline_objects: Option<&[ParagraphInlineObject]>,
) -> Result<(), ParagraphError> {
    if capabilities.supports(ParagraphCapability::SelectionGeometry)
        && !capabilities.supports(ParagraphCapability::ClusterMapping)
    {
        return Err(ParagraphError::invalid_result(
            "capabilities",
            "selection geometry requires cluster mapping",
        ));
    }

    validate_per_line_geometry(
        capabilities,
        ParagraphCapability::HitTesting,
        "hit_regions",
        geometry,
        |line_index| {
            geometry
                .hit_regions
                .iter()
                .any(|region| region.line_index == line_index)
        },
    )?;
    validate_per_line_geometry(
        capabilities,
        ParagraphCapability::CaretGeometry,
        "carets",
        geometry,
        |line_index| {
            geometry
                .carets
                .iter()
                .any(|caret| caret.line_index == line_index)
        },
    )?;

    let inline_ranges = expected_inline_objects
        .map(|objects| {
            objects
                .iter()
                .map(|object| object.range)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            geometry
                .inline_boxes
                .iter()
                .map(|object| object.range)
                .collect()
        });
    if capabilities.supports(ParagraphCapability::ClusterMapping) {
        for (line_index, line) in geometry.lines.iter().enumerate() {
            if line_has_regular_source(text, line.range, &inline_ranges)
                && !geometry
                    .clusters
                    .iter()
                    .any(|cluster| cluster.line_index == line_index)
            {
                return Err(ParagraphError::invalid_result(
                    "clusters",
                    format!(
                        "cluster mapping is claimed but visible line {line_index} has no cluster geometry"
                    ),
                ));
            }
        }
    }

    let supports_inline_objects = capabilities.supports(ParagraphCapability::InlineObjects);
    if !supports_inline_objects && !geometry.inline_boxes.is_empty() {
        return Err(ParagraphError::invalid_result(
            "inline_boxes",
            "inline object geometry was returned without the inline-object capability",
        ));
    }
    if let Some(expected) = expected_inline_objects {
        for output in &geometry.inline_boxes {
            if !expected
                .iter()
                .any(|input| input.id == output.id && input.range == output.range)
            {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes",
                    format!(
                        "inline object {} does not match an input object with the same range",
                        output.id
                    ),
                ));
            }
        }
        for input in expected.iter().filter(|input| {
            geometry
                .lines
                .iter()
                .any(|line| line.range.contains_range(input.range))
        }) {
            if !geometry
                .inline_boxes
                .iter()
                .any(|output| output.id == input.id && output.range == input.range)
            {
                return Err(ParagraphError::invalid_result(
                    "inline_boxes",
                    format!(
                        "visible inline object {} has no positioned output geometry",
                        input.id
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn validate_per_line_geometry(
    capabilities: ParagraphCapabilities,
    capability: ParagraphCapability,
    field: &'static str,
    geometry: &ParagraphGeometry,
    has_geometry: impl Fn(usize) -> bool,
) -> Result<(), ParagraphError> {
    if !capabilities.supports(capability) {
        return Ok(());
    }
    if let Some(line_index) = (0..geometry.lines.len()).find(|&index| !has_geometry(index)) {
        return Err(ParagraphError::invalid_result(
            field,
            format!(
                "{capability:?} is claimed but visible line {line_index} has no {field} geometry"
            ),
        ));
    }
    Ok(())
}

fn line_has_regular_source(text: &str, line: Utf8Range, inline_ranges: &[Utf8Range]) -> bool {
    let start = line.start().byte_offset();
    let end = line.end().byte_offset();
    text[start..end].char_indices().any(|(offset, character)| {
        let source_index = Utf8Index::new(start + offset);
        character != '\r'
            && character != '\n'
            && !inline_ranges
                .iter()
                .any(|range| range.contains(source_index))
    })
}

fn validate_unresolved_glyphs(
    text: &str,
    capabilities: ParagraphCapabilities,
    unresolved_glyphs: &[ParagraphUnresolvedGlyph],
) -> Result<(), ParagraphError> {
    if !unresolved_glyphs.is_empty()
        && !capabilities.supports(ParagraphCapability::UnresolvedGlyphDiagnostics)
    {
        return Err(ParagraphError::invalid_result(
            "unresolved_glyphs",
            "unresolved glyph diagnostics were returned without the corresponding capability",
        ));
    }
    for unresolved in unresolved_glyphs {
        validate_result_range(unresolved.range, text, "unresolved_glyphs.range")?;
        if unresolved.range.is_empty() {
            return Err(ParagraphError::invalid_result(
                "unresolved_glyphs.range",
                "unresolved glyph ranges cannot be empty",
            ));
        }
    }
    Ok(())
}

/// Stable identity for all shaping inputs represented by a backend cache entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParagraphCacheKey(u128);

impl ParagraphCacheKey {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u128 {
        self.0
    }
}

/// An opaque Fission identity for backend-owned immutable draw data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParagraphDrawDataId(u64);

impl ParagraphDrawDataId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One immutable measured result, including all interactive geometry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParagraphResult {
    text: Box<str>,
    cache_key: ParagraphCacheKey,
    capabilities: ParagraphCapabilities,
    geometry: ParagraphGeometry,
    unresolved_glyphs: Box<[ParagraphUnresolvedGlyph]>,
    draw_data: Option<ParagraphDrawDataId>,
}

impl<'de> Deserialize<'de> for ParagraphResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedParagraphResult {
            text: Box<str>,
            cache_key: ParagraphCacheKey,
            capabilities: ParagraphCapabilities,
            geometry: ParagraphGeometry,
            unresolved_glyphs: Box<[ParagraphUnresolvedGlyph]>,
            draw_data: Option<ParagraphDrawDataId>,
        }

        let result = SerializedParagraphResult::deserialize(deserializer)?;
        result
            .geometry
            .validate(&result.text)
            .map_err(de::Error::custom)?;
        validate_capability_geometry(&result.text, result.capabilities, &result.geometry, None)
            .map_err(de::Error::custom)?;
        validate_unresolved_glyphs(&result.text, result.capabilities, &result.unresolved_glyphs)
            .map_err(de::Error::custom)?;

        Ok(Self {
            text: result.text,
            cache_key: result.cache_key,
            capabilities: result.capabilities,
            geometry: result.geometry,
            unresolved_glyphs: result.unresolved_glyphs,
            draw_data: result.draw_data,
        })
    }
}

impl ParagraphResult {
    pub fn new(
        description: &ParagraphDescription,
        cache_key: ParagraphCacheKey,
        capabilities: ParagraphCapabilities,
        geometry: ParagraphGeometry,
        unresolved_glyphs: impl Into<Box<[ParagraphUnresolvedGlyph]>>,
    ) -> Result<Self, ParagraphError> {
        description.validate()?;
        capabilities
            .require_all(description.required_capabilities())
            .map_err(ParagraphError::UnsupportedCapability)?;
        geometry.validate(&description.text)?;
        validate_capability_geometry(
            &description.text,
            capabilities,
            &geometry,
            Some(&description.inline_objects),
        )?;

        let unresolved_glyphs = unresolved_glyphs.into();
        validate_unresolved_glyphs(&description.text, capabilities, &unresolved_glyphs)?;

        Ok(Self {
            text: description.text.clone().into_boxed_str(),
            cache_key,
            capabilities,
            geometry,
            unresolved_glyphs,
            draw_data: None,
        })
    }

    pub fn with_draw_data(mut self, draw_data: ParagraphDrawDataId) -> Self {
        self.draw_data = Some(draw_data);
        self
    }

    pub const fn cache_key(&self) -> ParagraphCacheKey {
        self.cache_key
    }

    /// Returns the UTF-8 source whose offsets all result geometry references.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn capabilities(&self) -> ParagraphCapabilities {
        self.capabilities
    }

    pub const fn geometry(&self) -> &ParagraphGeometry {
        &self.geometry
    }

    pub fn unresolved_glyphs(&self) -> &[ParagraphUnresolvedGlyph] {
        &self.unresolved_glyphs
    }

    pub const fn draw_data(&self) -> Option<ParagraphDrawDataId> {
        self.draw_data
    }

    /// Resolves a point without calling back into a backend-private paragraph.
    pub fn hit_test(&self, point: LayoutPoint) -> Result<ParagraphHitTest, ParagraphError> {
        self.capabilities
            .require(ParagraphCapability::HitTesting)
            .map_err(ParagraphError::UnsupportedCapability)?;

        let (region, is_inside) = self
            .geometry
            .hit_regions
            .iter()
            .find(|region| region.rect.contains(point))
            .map(|region| (region, true))
            .or_else(|| {
                self.geometry
                    .hit_regions
                    .iter()
                    .min_by(|left, right| {
                        distance_to_rect(point, left.rect)
                            .total_cmp(&distance_to_rect(point, right.rect))
                    })
                    .map(|region| (region, false))
            })
            .ok_or_else(|| ParagraphError::missing_geometry("hit testing"))?;

        Ok(ParagraphHitTest {
            index: region.index,
            affinity: region.affinity,
            line_index: region.line_index,
            is_inside,
        })
    }

    pub fn caret(
        &self,
        index: Utf8Index,
        affinity: ParagraphAffinity,
    ) -> Result<Option<&ParagraphCaret>, ParagraphError> {
        self.capabilities
            .require(ParagraphCapability::CaretGeometry)
            .map_err(ParagraphError::UnsupportedCapability)?;
        if !index.is_boundary_in(&self.text) {
            return Err(ParagraphError::invalid_query(
                "caret.index",
                format!(
                    "offset {} is not a UTF-8 boundary for {} bytes of text",
                    index.byte_offset(),
                    self.text.len()
                ),
            ));
        }
        Ok(self
            .geometry
            .carets
            .iter()
            .find(|caret| caret.index == index && caret.affinity == affinity)
            .or_else(|| {
                self.geometry
                    .carets
                    .iter()
                    .find(|caret| caret.index == index)
            }))
    }

    /// Returns visible selection fragments for an arbitrary source range.
    ///
    /// Selection is derived from the same immutable cluster and inline-object
    /// geometry used by hit testing. If an endpoint falls inside a shaped
    /// cluster, the visual result is intentionally snapped to that complete
    /// cluster because it is the smallest geometry unit the paragraph engine
    /// exposes.
    pub fn selection_boxes(
        &self,
        range: Utf8Range,
    ) -> Result<Vec<ParagraphSelectionBox>, ParagraphError> {
        self.capabilities
            .require(ParagraphCapability::SelectionGeometry)
            .map_err(ParagraphError::UnsupportedCapability)?;
        self.capabilities
            .require(ParagraphCapability::ClusterMapping)
            .map_err(ParagraphError::UnsupportedCapability)?;
        if !range.start().is_boundary_in(&self.text) || !range.end().is_boundary_in(&self.text) {
            return Err(ParagraphError::invalid_query(
                "selection.range",
                format!(
                    "range {}..{} is not valid for {} UTF-8 bytes",
                    range.start().byte_offset(),
                    range.end().byte_offset(),
                    self.text.len()
                ),
            ));
        }
        if range.is_empty() {
            return Ok(Vec::new());
        }

        let mut fragments = self
            .geometry
            .clusters
            .iter()
            .filter(|cluster| cluster.range.intersects(range))
            .map(|cluster| {
                (
                    cluster.line_index,
                    ParagraphSelectionBox {
                        range: cluster.range,
                        rect: cluster.rect,
                        direction: cluster.direction,
                    },
                )
            })
            .collect::<Vec<_>>();

        for inline in self
            .geometry
            .inline_boxes
            .iter()
            .filter(|inline| inline.range.intersects(range))
        {
            let line_index = self
                .geometry
                .lines
                .iter()
                .position(|line| line.range.intersects(inline.range))
                .expect("validated inline box must belong to a visible line");
            let direction = self.geometry.lines[line_index].direction;
            fragments.push((
                line_index,
                ParagraphSelectionBox {
                    range: inline.range,
                    rect: inline.rect,
                    direction,
                },
            ));
        }

        fragments.sort_by(|(left_line, left), (right_line, right)| {
            left_line
                .cmp(right_line)
                .then_with(|| left.rect.x().total_cmp(&right.rect.x()))
        });
        Ok(fragments
            .into_iter()
            .map(|(_, fragment)| fragment)
            .collect())
    }
}

fn distance_to_rect(point: LayoutPoint, rect: LayoutRect) -> f32 {
    let dx = if point.x < rect.x() {
        rect.x() - point.x
    } else if point.x > rect.right() {
        point.x - rect.right()
    } else {
        0.0
    };
    let dy = if point.y < rect.y() {
        rect.y() - point.y
    } else if point.y > rect.bottom() {
        point.y - rect.bottom()
    } else {
        0.0
    };
    dx.mul_add(dx, dy * dy)
}

/// Backend-neutral paragraph engine implemented by Parley, SkParagraph, or a
/// future Fission-owned text system.
pub trait ParagraphEngine: Send + Sync {
    fn capabilities(&self) -> ParagraphCapabilities;

    fn layout(&self, description: &ParagraphDescription)
        -> Result<ParagraphResult, ParagraphError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParagraphError {
    InvalidDescription {
        field: &'static str,
        details: String,
    },
    UnsupportedCapability(ParagraphCapabilityError),
    InvalidResult {
        field: &'static str,
        details: String,
    },
    InvalidQuery {
        field: &'static str,
        details: String,
    },
    MissingGeometry {
        operation: &'static str,
    },
    Backend {
        backend: String,
        details: String,
    },
}

impl ParagraphError {
    pub fn invalid_description(field: &'static str, details: impl Into<String>) -> Self {
        Self::InvalidDescription {
            field,
            details: details.into(),
        }
    }

    pub fn invalid_result(field: &'static str, details: impl Into<String>) -> Self {
        Self::InvalidResult {
            field,
            details: details.into(),
        }
    }

    pub fn invalid_query(field: &'static str, details: impl Into<String>) -> Self {
        Self::InvalidQuery {
            field,
            details: details.into(),
        }
    }

    pub const fn missing_geometry(operation: &'static str) -> Self {
        Self::MissingGeometry { operation }
    }

    pub fn backend(backend: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Backend {
            backend: backend.into(),
            details: details.into(),
        }
    }
}

impl fmt::Display for ParagraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescription { field, details } => {
                write!(
                    formatter,
                    "invalid paragraph description field {field}: {details}"
                )
            }
            Self::UnsupportedCapability(error) => error.fmt(formatter),
            Self::InvalidResult { field, details } => {
                write!(
                    formatter,
                    "invalid paragraph result field {field}: {details}"
                )
            }
            Self::InvalidQuery { field, details } => {
                write!(
                    formatter,
                    "invalid paragraph query field {field}: {details}"
                )
            }
            Self::MissingGeometry { operation } => {
                write!(
                    formatter,
                    "paragraph result has no geometry for {operation}"
                )
            }
            Self::Backend { backend, details } => {
                write!(formatter, "{backend} paragraph backend failed: {details}")
            }
        }
    }
}

impl Error for ParagraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnsupportedCapability(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ParagraphCapabilityError> for ParagraphError {
    fn from(value: ParagraphCapabilityError) -> Self {
        Self::UnsupportedCapability(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_ir::op::{Color, FontStyle};

    fn range(start: usize, end: usize) -> Utf8Range {
        Utf8Range::from_byte_offsets(start, end).unwrap()
    }

    fn style() -> TextStyle {
        TextStyle {
            font_size: 16.0,
            color: Color::BLACK,
            underline: false,
            font_family: Some("Inter".into()),
            locale: Some("en-GB".into()),
            font_weight: 400,
            font_style: FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
        }
    }

    fn description(text: &str) -> ParagraphDescription {
        ParagraphDescription::new(
            text,
            vec![ParagraphStyleRun::new(range(0, text.len()), style())],
            TextParagraphStyle::default(),
            Some(300.0),
        )
    }

    #[test]
    fn utf8_ranges_reject_mid_scalar_boundaries() {
        let text = "aé日";

        assert!(range(0, text.len()).validate_in(text, "text").is_ok());
        assert!(range(1, 2).validate_in(text, "text").is_err());
    }

    #[test]
    fn normalized_runs_must_cover_text_without_gaps() {
        let mut description = description("hello");
        description.style_runs = vec![ParagraphStyleRun::new(range(1, 5), style())];

        assert!(matches!(
            description.validate(),
            Err(ParagraphError::InvalidDescription {
                field: "style_runs",
                ..
            })
        ));
    }

    #[test]
    fn utf8_range_deserialization_preserves_ordering_invariant() {
        let error = serde_json::from_str::<Utf8Range>(r#"{"start":4,"end":2}"#)
            .unwrap_err()
            .to_string();

        assert!(error.contains("UTF-8 range start 4 exceeds end 2"));
    }

    #[test]
    fn normalized_inline_objects_must_be_ordered_and_non_overlapping() {
        let text = "A\u{fffc}B\u{fffc}C";
        let mut description = description(text);
        description.inline_objects = vec![
            ParagraphInlineObject {
                id: 1,
                range: range(5, 8),
                size: LayoutSize::new(10.0, 10.0),
                baseline: 10.0,
            },
            ParagraphInlineObject {
                id: 2,
                range: range(1, 4),
                size: LayoutSize::new(10.0, 10.0),
                baseline: 10.0,
            },
        ];

        assert!(matches!(
            description.validate(),
            Err(ParagraphError::InvalidDescription {
                field: "inline_objects.range",
                ..
            })
        ));
    }

    #[test]
    fn capability_failures_are_explicit() {
        let description = description("hello");
        let geometry = ParagraphGeometry::new(LayoutSize::new(50.0, 20.0));

        let error = ParagraphResult::new(
            &description,
            ParagraphCacheKey::new(1),
            ParagraphCapabilities::NONE,
            geometry,
            Vec::<ParagraphUnresolvedGlyph>::new(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ParagraphError::UnsupportedCapability(ParagraphCapabilityError {
                capability: ParagraphCapability::BidirectionalText
            })
        ));
    }

    #[test]
    fn one_result_answers_hit_caret_and_selection_queries() {
        let mut description = description("hello");
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let full = range(0, 5);
        let geometry = ParagraphGeometry::new(LayoutSize::new(50.0, 20.0))
            .with_lines(vec![ParagraphLine {
                range: full,
                rect: LayoutRect::new(0.0, 0.0, 50.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 3.0,
                leading: 0.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }])
            .with_carets(vec![ParagraphCaret {
                index: Utf8Index::new(0),
                affinity: ParagraphAffinity::Downstream,
                rect: LayoutRect::new(0.0, 0.0, 1.0, 20.0),
                line_index: 0,
            }])
            .with_clusters(vec![ParagraphCluster {
                range: full,
                rect: LayoutRect::new(0.0, 0.0, 50.0, 20.0),
                line_index: 0,
                direction: ParagraphDirection::LeftToRight,
                starts_grapheme: true,
                starts_word: true,
            }])
            .with_hit_regions(vec![ParagraphHitRegion {
                rect: LayoutRect::new(0.0, 0.0, 25.0, 20.0),
                index: Utf8Index::new(0),
                affinity: ParagraphAffinity::Downstream,
                line_index: 0,
            }]);
        let capabilities = ParagraphCapabilities::NONE
            .with(ParagraphCapability::HitTesting)
            .with(ParagraphCapability::CaretGeometry)
            .with(ParagraphCapability::ClusterMapping)
            .with(ParagraphCapability::SelectionGeometry);
        let result = ParagraphResult::new(
            &description,
            ParagraphCacheKey::new(9),
            capabilities,
            geometry,
            Vec::<ParagraphUnresolvedGlyph>::new(),
        )
        .unwrap();

        assert!(
            result
                .hit_test(LayoutPoint::new(5.0, 5.0))
                .unwrap()
                .is_inside
        );
        assert!(result
            .caret(Utf8Index::new(0), ParagraphAffinity::Downstream)
            .unwrap()
            .is_some());
        assert_eq!(result.selection_boxes(full).unwrap().len(), 1);
    }

    #[test]
    fn selection_geometry_serves_ranges_not_present_in_the_description() {
        let mut description = description("hello");
        description.paragraph_style.text_direction = TextDirection::Ltr;
        assert!(description.selection.is_none());

        let full = range(0, 5);
        let geometry = ParagraphGeometry::new(LayoutSize::new(50.0, 20.0))
            .with_lines(vec![ParagraphLine {
                range: full,
                rect: LayoutRect::new(0.0, 0.0, 50.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 3.0,
                leading: 0.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }])
            .with_clusters(vec![
                ParagraphCluster {
                    range: range(0, 2),
                    rect: LayoutRect::new(0.0, 0.0, 20.0, 20.0),
                    line_index: 0,
                    direction: ParagraphDirection::LeftToRight,
                    starts_grapheme: true,
                    starts_word: true,
                },
                ParagraphCluster {
                    range: range(2, 5),
                    rect: LayoutRect::new(20.0, 0.0, 30.0, 20.0),
                    line_index: 0,
                    direction: ParagraphDirection::LeftToRight,
                    starts_grapheme: true,
                    starts_word: false,
                },
            ]);
        let result = ParagraphResult::new(
            &description,
            ParagraphCacheKey::new(10),
            ParagraphCapabilities::NONE
                .with(ParagraphCapability::ClusterMapping)
                .with(ParagraphCapability::SelectionGeometry),
            geometry,
            Vec::<ParagraphUnresolvedGlyph>::new(),
        )
        .unwrap();

        let fragments = result.selection_boxes(range(1, 4)).unwrap();
        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].range, range(0, 2));
        assert_eq!(fragments[1].range, range(2, 5));
    }

    #[test]
    fn result_rejects_non_utf8_geometry_indices() {
        let text = "é";
        let mut description = description(text);
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let geometry = ParagraphGeometry::new(LayoutSize::new(20.0, 20.0))
            .with_lines(vec![ParagraphLine {
                range: range(0, text.len()),
                rect: LayoutRect::new(0.0, 0.0, 20.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 3.0,
                leading: 0.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }])
            .with_carets(vec![ParagraphCaret {
                index: Utf8Index::new(1),
                affinity: ParagraphAffinity::Downstream,
                rect: LayoutRect::new(0.0, 0.0, 1.0, 20.0),
                line_index: 0,
            }]);

        assert!(matches!(
            ParagraphResult::new(
                &description,
                ParagraphCacheKey::new(1),
                ParagraphCapabilities::NONE.with(ParagraphCapability::CaretGeometry),
                geometry,
                Vec::<ParagraphUnresolvedGlyph>::new(),
            ),
            Err(ParagraphError::InvalidResult {
                field: "carets.index",
                ..
            })
        ));
    }

    #[test]
    fn claimed_interaction_capabilities_require_geometry_for_every_visible_line() {
        let mut description = description("hello");
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let line = ParagraphLine {
            range: range(0, 5),
            rect: LayoutRect::new(0.0, 0.0, 50.0, 20.0),
            baseline: 15.0,
            ascent: 12.0,
            descent: 3.0,
            leading: 0.0,
            hard_break: false,
            direction: ParagraphDirection::LeftToRight,
        };

        for (capability, field) in [
            (ParagraphCapability::HitTesting, "hit_regions"),
            (ParagraphCapability::CaretGeometry, "carets"),
        ] {
            let error = ParagraphResult::new(
                &description,
                ParagraphCacheKey::new(1),
                ParagraphCapabilities::NONE.with(capability),
                ParagraphGeometry::new(LayoutSize::new(50.0, 20.0)).with_lines(vec![line]),
                Vec::<ParagraphUnresolvedGlyph>::new(),
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ParagraphError::InvalidResult {
                    field: actual,
                    ..
                } if actual == field
            ));
        }
    }

    #[test]
    fn selection_capability_requires_cluster_mapping() {
        let mut description = description("hello");
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let geometry =
            ParagraphGeometry::new(LayoutSize::new(50.0, 20.0)).with_lines(vec![ParagraphLine {
                range: range(0, 5),
                rect: LayoutRect::new(0.0, 0.0, 50.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 3.0,
                leading: 0.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }]);

        assert!(matches!(
            ParagraphResult::new(
                &description,
                ParagraphCacheKey::new(1),
                ParagraphCapabilities::NONE.with(ParagraphCapability::SelectionGeometry),
                geometry,
                Vec::<ParagraphUnresolvedGlyph>::new(),
            ),
            Err(ParagraphError::InvalidResult {
                field: "capabilities",
                ..
            })
        ));
    }

    #[test]
    fn visible_inline_objects_require_matching_output_geometry() {
        let text = "A\u{fffc}B";
        let mut description = description(text);
        description.paragraph_style.text_direction = TextDirection::Ltr;
        description.inline_objects.push(ParagraphInlineObject {
            id: 7,
            range: range(1, 4),
            size: LayoutSize::new(12.0, 10.0),
            baseline: 10.0,
        });
        let geometry =
            ParagraphGeometry::new(LayoutSize::new(40.0, 20.0)).with_lines(vec![ParagraphLine {
                range: range(0, text.len()),
                rect: LayoutRect::new(0.0, 0.0, 40.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 3.0,
                leading: 0.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }]);

        assert!(matches!(
            ParagraphResult::new(
                &description,
                ParagraphCacheKey::new(1),
                ParagraphCapabilities::NONE.with(ParagraphCapability::InlineObjects),
                geometry,
                Vec::<ParagraphUnresolvedGlyph>::new(),
            ),
            Err(ParagraphError::InvalidResult {
                field: "inline_boxes",
                ..
            })
        ));
    }

    #[test]
    fn paragraph_result_deserialization_revalidates_invariants() {
        let mut description = description("hello");
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let result = ParagraphResult::new(
            &description,
            ParagraphCacheKey::new(1),
            ParagraphCapabilities::NONE,
            ParagraphGeometry::new(LayoutSize::new(50.0, 20.0)).with_lines(vec![ParagraphLine {
                range: range(0, 5),
                rect: LayoutRect::new(0.0, 0.0, 50.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 3.0,
                leading: 0.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }]),
            Vec::<ParagraphUnresolvedGlyph>::new(),
        )
        .unwrap();
        let serialized = serde_json::to_value(result).unwrap();
        let mut invalid_geometry = serialized.clone();
        invalid_geometry["geometry"]["size"]["width"] = serde_json::Value::from(-1.0);

        let error = serde_json::from_value::<ParagraphResult>(invalid_geometry)
            .unwrap_err()
            .to_string();
        assert!(error.contains("paragraph dimensions must be finite, non-negative, and ordered"));

        let mut invalid_capability = serialized;
        invalid_capability["capabilities"] =
            serde_json::Value::from(ParagraphCapability::HitTesting.bit());
        let error = serde_json::from_value::<ParagraphResult>(invalid_capability)
            .unwrap_err()
            .to_string();
        assert!(error.contains("visible line 0 has no hit_regions geometry"));
    }
}
