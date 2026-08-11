use fission_ir::op::{BoxStyle, Length};

use crate::{BoxConstraints, LayoutOp, LayoutSize, LayoutUnit};

pub(crate) fn resolve_length(
    length: &Length,
    reference: LayoutUnit,
    viewport: LayoutSize,
) -> Option<LayoutUnit> {
    length
        .resolve(reference, viewport.width, viewport.height)
        .map(|value| value.max(0.0))
}

pub(crate) fn length_requires_measurement(length: &Length) -> bool {
    match length {
        Length::FitContent(_) | Length::MinContent | Length::MaxContent => true,
        Length::Add(left, right) | Length::Subtract(left, right) => {
            length_requires_measurement(left) || length_requires_measurement(right)
        }
        Length::Min(values) | Length::Max(values) => values.iter().any(length_requires_measurement),
        Length::Clamp {
            min,
            preferred,
            max,
        } => {
            length_requires_measurement(min)
                || length_requires_measurement(preferred)
                || length_requires_measurement(max)
        }
        Length::Points(_)
        | Length::Percent(_)
        | Length::ViewportWidth(_)
        | Length::ViewportHeight(_)
        | Length::Auto => false,
    }
}

pub(crate) fn resolve_measured_length(
    length: &Length,
    reference: LayoutUnit,
    viewport: LayoutSize,
    min_content: LayoutUnit,
    max_content: LayoutUnit,
) -> Option<LayoutUnit> {
    let resolved = match length {
        Length::MinContent => min_content,
        Length::MaxContent => max_content,
        Length::FitContent(limit) => {
            let limit = limit
                .as_deref()
                .and_then(|limit| {
                    resolve_measured_length(limit, reference, viewport, min_content, max_content)
                })
                .unwrap_or(reference);
            max_content.min(min_content.max(limit))
        }
        Length::Add(left, right) => {
            resolve_measured_length(left, reference, viewport, min_content, max_content)?
                + resolve_measured_length(right, reference, viewport, min_content, max_content)?
        }
        Length::Subtract(left, right) => {
            resolve_measured_length(left, reference, viewport, min_content, max_content)?
                - resolve_measured_length(right, reference, viewport, min_content, max_content)?
        }
        Length::Min(values) => values
            .iter()
            .map(|value| {
                resolve_measured_length(value, reference, viewport, min_content, max_content)
            })
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .reduce(LayoutUnit::min)?,
        Length::Max(values) => values
            .iter()
            .map(|value| {
                resolve_measured_length(value, reference, viewport, min_content, max_content)
            })
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .reduce(LayoutUnit::max)?,
        Length::Clamp {
            min,
            preferred,
            max,
        } => {
            let minimum =
                resolve_measured_length(min, reference, viewport, min_content, max_content)?;
            let maximum =
                resolve_measured_length(max, reference, viewport, min_content, max_content)?;
            resolve_measured_length(preferred, reference, viewport, min_content, max_content)?
                .clamp(minimum.min(maximum), minimum.max(maximum))
        }
        Length::Auto => return None,
        Length::Points(_)
        | Length::Percent(_)
        | Length::ViewportWidth(_)
        | Length::ViewportHeight(_) => {
            length.resolve(reference, viewport.width, viewport.height)?
        }
    };
    resolved.is_finite().then_some(resolved.max(0.0))
}

pub(crate) fn resolve_box_style(
    style: &BoxStyle,
    constraints: BoxConstraints,
    viewport: LayoutSize,
) -> LayoutOp {
    let horizontal_reference = constraints.max_w;
    let vertical_reference = constraints.max_h;
    let padding = style
        .padding
        .as_ref()
        .map(|padding| {
            [
                resolve_length(&padding[0], horizontal_reference, viewport).unwrap_or(0.0),
                resolve_length(&padding[1], horizontal_reference, viewport).unwrap_or(0.0),
                resolve_length(&padding[2], vertical_reference, viewport).unwrap_or(0.0),
                resolve_length(&padding[3], vertical_reference, viewport).unwrap_or(0.0),
            ]
        })
        .unwrap_or([0.0; 4]);
    let fit_content_limit = |length: &Option<Length>, reference| match length {
        Some(Length::FitContent(Some(limit))) => resolve_length(limit, reference, viewport),
        _ => None,
    };
    let resolved_max_width = style
        .max_width
        .as_ref()
        .and_then(|value| resolve_length(value, horizontal_reference, viewport));
    let resolved_max_height = style
        .max_height
        .as_ref()
        .and_then(|value| resolve_length(value, vertical_reference, viewport));
    LayoutOp::Box {
        width: style.width.as_ref().and_then(|value| {
            (!matches!(value, Length::FitContent(_)))
                .then(|| resolve_length(value, horizontal_reference, viewport))
                .flatten()
        }),
        height: style.height.as_ref().and_then(|value| {
            (!matches!(value, Length::FitContent(_)))
                .then(|| resolve_length(value, vertical_reference, viewport))
                .flatten()
        }),
        min_width: style
            .min_width
            .as_ref()
            .and_then(|value| resolve_length(value, horizontal_reference, viewport)),
        max_width: match (
            resolved_max_width,
            fit_content_limit(&style.width, horizontal_reference),
        ) {
            (Some(maximum), Some(fit)) => Some(maximum.min(fit)),
            (maximum, fit) => maximum.or(fit),
        },
        min_height: style
            .min_height
            .as_ref()
            .and_then(|value| resolve_length(value, vertical_reference, viewport)),
        max_height: match (
            resolved_max_height,
            fit_content_limit(&style.height, vertical_reference),
        ) {
            (Some(maximum), Some(fit)) => Some(maximum.min(fit)),
            (maximum, fit) => maximum.or(fit),
        },
        padding,
        flex_grow: style.flex_grow.map(|value| value.0).unwrap_or(0.0),
        flex_shrink: style.flex_shrink.map(|value| value.0).unwrap_or(1.0),
        aspect_ratio: style.aspect_ratio.map(|value| value.0),
    }
}
