use anyhow::Result;
use fission_ir::op::Length;
use fission_ir::{LayoutOp, WidgetId};
use std::collections::HashMap;

use super::graph::MeasureCacheKey;
use super::LayoutEngine;
use crate::style::{length_requires_measurement, resolve_measured_length};
use crate::{
    BoxConstraints, LayoutInputNode, LayoutNodeGeometry, LayoutPoint, LayoutSize, ScrollDataSource,
};

impl LayoutEngine {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn layout_rich_text_content(
        &self,
        node_id: WidgetId,
        node: &LayoutInputNode,
        layout_op: &LayoutOp,
        constraints: BoxConstraints,
        origin: LayoutPoint,
        flow_children: &[WidgetId],
        rich_text_inline_children: bool,
        content_size: &mut LayoutSize,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<WidgetId, BoxConstraints>,
        measure_cache: &mut HashMap<MeasureCacheKey, LayoutSize>,
        scroll_source: &impl ScrollDataSource,
        record: bool,
        depth: usize,
    ) -> Result<Option<LayoutSize>> {
        let Some(runs) = &node.rich_text else {
            return Ok(None);
        };
        let Some(measurer) = &self.measurer else {
            return Ok(None);
        };
        let (mut text_constraints, text_padding) = match layout_op {
            LayoutOp::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                ..
            } => (
                constraints
                    .apply_min_max(*min_width, *max_width, *min_height, *max_height)
                    .tighten(*width, *height),
                *padding,
            ),
            _ => (constraints, [0.0; 4]),
        };
        let text_inner_constraints = text_constraints.deflate(text_padding);
        let intrinsic_width = match &node.op {
            LayoutOp::StyledBox { style, .. } => style.width.as_ref(),
            _ => None,
        };
        let avail_w = match intrinsic_width {
            Some(Length::MaxContent) => None,
            Some(Length::MinContent) => Some(
                runs.iter()
                    .flat_map(|run| {
                        run.text.split_whitespace().map(move |word| {
                            measurer.measure(word, run.style.font_size, None).0
                                + run.style.letter_spacing
                                    * word.chars().count().saturating_sub(1) as f32
                        })
                    })
                    .fold(0.0, f32::max),
            ),
            _ => text_inner_constraints
                .is_width_bounded()
                .then_some(text_inner_constraints.max_w),
        };
        let rich_layout = measurer.layout_rich_text(runs, avail_w);
        let text_content = LayoutSize::new(
            rich_layout.width + text_padding[0] + text_padding[1],
            rich_layout.height + text_padding[2] + text_padding[3],
        );
        if let LayoutOp::StyledBox { style, .. } = &node.op {
            let available = if constraints.max_h.is_finite() {
                constraints.max_h
            } else {
                text_content.height
            };
            let resolve_intrinsic_height = |length: &Option<Length>| {
                length
                    .as_ref()
                    .filter(|length| length_requires_measurement(length))
                    .and_then(|length| {
                        resolve_measured_length(
                            length,
                            available,
                            self.active_viewport,
                            text_content.height,
                            text_content.height,
                        )
                    })
            };
            text_constraints = text_constraints.apply_min_max(
                None,
                None,
                resolve_intrinsic_height(&style.min_height),
                resolve_intrinsic_height(&style.max_height),
            );
            text_constraints =
                text_constraints.tighten(None, resolve_intrinsic_height(&style.height));
        }
        let measured = text_constraints.constrain(text_content);
        if rich_text_inline_children && rich_layout.inline_boxes.len() == flow_children.len() {
            let result = self.record_geometry(node_id, origin, measured, text_content, out, record);
            if record {
                let mut inline_boxes = rich_layout.inline_boxes;
                inline_boxes.sort_by_key(|inline_box| inline_box.id);
                for (child_id, inline_box) in flow_children.iter().zip(inline_boxes.iter()) {
                    self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::tight(LayoutSize::new(inline_box.width, inline_box.height)),
                        LayoutPoint::new(
                            origin.x + text_padding[0] + inline_box.x,
                            origin.y + text_padding[2] + inline_box.y,
                        ),
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
            }
            if !record {
                measure_cache.insert(MeasureCacheKey::new(node_id, constraints), result);
            }
            return Ok(Some(result));
        }
        if node.children_ids.is_empty() {
            let result = self.record_geometry(node_id, origin, measured, text_content, out, record);
            if !record {
                measure_cache.insert(MeasureCacheKey::new(node_id, constraints), result);
            }
            return Ok(Some(result));
        }
        content_size.width = content_size.width.max(text_content.width);
        content_size.height = content_size.height.max(text_content.height);
        Ok(None)
    }
}
