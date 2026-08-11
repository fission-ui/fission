mod cache_key;
mod geometry;
mod source_map;

use fission_ir::op::{Color as IrColor, TextStyle as IrTextStyle};
use fission_layout::{
    ParagraphCapabilities, ParagraphCapability, ParagraphDescription, ParagraphEngine,
    ParagraphError, ParagraphResult, ParagraphUnresolvedGlyph,
};
use fission_render::{Color as RenderColor, LayoutRect as RenderLayoutRect};

use crate::paragraph::{
    paragraph_alignment, paragraph_alignment_options, paragraph_alignment_width,
    prepare_paragraph_layout,
};
use crate::text::{RichInlineBox, RichShapingStyle, VelloTextMeasurer};

use self::cache_key::paragraph_cache_key;
use self::geometry::build_geometry;
use self::source_map::ParagraphSourceMap;

const PARLEY_CAPABILITIES: ParagraphCapabilities = ParagraphCapabilities::NONE
    .with(ParagraphCapability::BidirectionalText)
    .with(ParagraphCapability::VariableFonts)
    .with(ParagraphCapability::FontFeatures)
    .with(ParagraphCapability::ClusterMapping)
    .with(ParagraphCapability::HitTesting)
    .with(ParagraphCapability::CaretGeometry)
    .with(ParagraphCapability::SelectionGeometry);

impl ParagraphEngine for VelloTextMeasurer {
    fn capabilities(&self) -> ParagraphCapabilities {
        PARLEY_CAPABILITIES
    }

    fn layout(
        &self,
        description: &ParagraphDescription,
    ) -> Result<ParagraphResult, ParagraphError> {
        description.validate()?;
        PARLEY_CAPABILITIES.require_all(description.required_capabilities())?;
        validate_parley_inputs(description)?;

        let source_map = ParagraphSourceMap::new(description)?;
        let (base_style, shaping_styles) =
            paragraph_styles(description, &source_map, self.default_family_name())?;
        let render_styles = shaping_styles
            .iter()
            .map(|span| (span.range.clone(), span.style.clone()))
            .collect::<Vec<_>>();
        let inline_boxes = description
            .inline_objects
            .iter()
            .map(|inline| RichInlineBox {
                id: inline.id,
                index: source_map.source_to_shaped(inline.range.start().byte_offset()),
                width: inline.size.width,
                height: inline.size.height,
            })
            .collect::<Vec<_>>();
        let prepared = prepare_paragraph_layout(
            source_map.text(),
            &base_style,
            description.paragraph_style,
            &inline_boxes,
            &render_styles,
            None,
        );
        let prepared_styles = apply_prepared_render_styles(
            &prepared.styles,
            &shaping_styles,
            prepared.text_byte_offset,
        );
        let wrapping_width = description
            .wrap
            .then_some(description.width_constraint)
            .flatten();
        let mut layout = (*self.layout_rich_with_shaping(
            &prepared.text,
            prepared.base_style.font_size,
            prepared.base_style.color,
            &prepared_styles,
            &prepared.inline_boxes,
            wrapping_width,
        ))
        .clone();

        let bounds_width = description
            .width_constraint
            .unwrap_or_else(|| layout.width().max(0.0));
        let bounds = RenderLayoutRect::new(0.0, 0.0, bounds_width, layout.height().max(0.0));
        if let Some(alignment_width) =
            paragraph_alignment_width(&layout, bounds, description.paragraph_style)
        {
            layout.align(
                Some(alignment_width),
                paragraph_alignment(description.paragraph_style.text_align),
                paragraph_alignment_options(description.paragraph_style.text_align),
            );
        }

        let geometry = build_geometry(description, &layout, &source_map, prepared.text_byte_offset);
        ParagraphResult::new(
            description,
            paragraph_cache_key(description, self.default_family_name()),
            PARLEY_CAPABILITIES,
            geometry,
            Vec::<ParagraphUnresolvedGlyph>::new(),
        )
    }
}

fn validate_parley_inputs(description: &ParagraphDescription) -> Result<(), ParagraphError> {
    for (run_index, run) in description.style_runs.iter().enumerate() {
        if let Some(feature) = run
            .features
            .iter()
            .find(|feature| feature.value > u16::MAX.into())
        {
            return Err(ParagraphError::invalid_description(
                "style_runs.features.value",
                format!(
                    "style run {run_index} feature tag {:#010x} has value {}, but Parley accepts values through {}",
                    feature.tag,
                    feature.value,
                    u16::MAX
                ),
            ));
        }
    }
    Ok(())
}

fn paragraph_styles(
    description: &ParagraphDescription,
    source_map: &ParagraphSourceMap,
    default_family: &str,
) -> Result<(fission_render::TextStyle, Vec<RichShapingStyle>), ParagraphError> {
    let base_style = description
        .style_runs
        .first()
        .map(|run| render_style(&run.style, description, default_family))
        .unwrap_or_else(default_render_style);
    let mut spans = Vec::with_capacity(description.style_runs.len());

    for run in &description.style_runs {
        let range = source_map.source_range_to_shaped(run.range);
        let features = run
            .features
            .iter()
            .map(|feature| {
                u16::try_from(feature.value)
                    .map(|value| (feature.tag, value))
                    .map_err(|_| {
                        ParagraphError::invalid_description(
                            "style_runs.features.value",
                            "feature value exceeds the Parley u16 range",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        spans.push(RichShapingStyle {
            range,
            style: render_style(&run.style, description, default_family),
            font_width: run.font_width,
            word_spacing: run.word_spacing,
            variations: run
                .variations
                .iter()
                .map(|variation| (variation.tag, variation.value))
                .collect(),
            features,
        });
    }

    Ok((base_style, spans))
}

fn render_style(
    style: &IrTextStyle,
    description: &ParagraphDescription,
    default_family: &str,
) -> fission_render::TextStyle {
    let font_family = if description.fallback_families.is_empty() {
        style.font_family.clone()
    } else {
        let primary = style.font_family.as_deref().unwrap_or(default_family);
        Some(
            std::iter::once(primary)
                .chain(description.fallback_families.iter().map(String::as_str))
                .filter(|family| !family.trim().is_empty())
                .collect::<Vec<_>>()
                .join(", "),
        )
    };
    fission_render::TextStyle {
        font_size: style.font_size,
        color: render_color(style.color),
        underline: style.underline,
        font_family,
        locale: style.locale.clone().or_else(|| description.locale.clone()),
        font_weight: style.font_weight,
        font_style: style.font_style,
        line_height: style.line_height,
        letter_spacing: style.letter_spacing,
        background_color: style.background_color.map(render_color),
    }
}

fn default_render_style() -> fission_render::TextStyle {
    fission_render::TextStyle {
        font_size: 14.0,
        color: RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        underline: false,
        font_family: None,
        locale: None,
        font_weight: 400,
        font_style: Default::default(),
        line_height: None,
        letter_spacing: 0.0,
        background_color: None,
    }
}

fn render_color(color: IrColor) -> RenderColor {
    RenderColor {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

fn apply_prepared_render_styles(
    prepared: &[(std::ops::Range<usize>, fission_render::TextStyle)],
    shaping: &[RichShapingStyle],
    text_byte_offset: usize,
) -> Vec<RichShapingStyle> {
    let has_direction_prefix = text_byte_offset != 0;
    let mut output = Vec::with_capacity(prepared.len());
    let mut prepared_iter = prepared.iter();

    if has_direction_prefix {
        if let Some((range, style)) = prepared_iter.next() {
            output.push(RichShapingStyle {
                range: range.clone(),
                style: style.clone(),
                font_width: 1.0,
                word_spacing: 0.0,
                variations: Vec::new(),
                features: Vec::new(),
            });
        }
    }

    output.extend(
        prepared_iter
            .zip(shaping)
            .map(|((range, style), shaping)| RichShapingStyle {
                range: range.clone(),
                style: style.clone(),
                font_width: shaping.font_width,
                word_spacing: shaping.word_spacing,
                variations: shaping.variations.clone(),
                features: shaping.features.clone(),
            }),
    );
    output
}

#[cfg(test)]
mod tests;
