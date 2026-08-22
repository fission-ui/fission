use anyhow::Result;
use fission_ir::op::Length;
use fission_ir::{FlexDirection as IrFlexDirection, FlexWrap as IrFlexWrap, LayoutOp, WidgetId};
use std::collections::HashMap;

use super::graph::MeasureCacheKey;
use super::LayoutEngine;
use crate::geometry::finite_or;
use crate::grid_tracks::IntrinsicAxis;
use crate::input::{has_explicit_cross_axis_size, has_explicit_main_axis_size};
use crate::style::{
    length_requires_measurement, resolve_box_style, resolve_length, resolve_measured_length,
};
use crate::{BoxConstraints, LayoutNodeGeometry, LayoutPoint, LayoutSize, ScrollDataSource};

impl LayoutEngine {
    pub(super) fn layout_node_constraints(
        &self,
        node_id: WidgetId,
        constraints: BoxConstraints,
        origin: LayoutPoint,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<WidgetId, BoxConstraints>,
        measure_cache: &mut HashMap<MeasureCacheKey, LayoutSize>,
        scroll_source: &impl ScrollDataSource,
        record: bool,
        depth: usize,
    ) -> Result<LayoutSize> {
        if depth > Self::MAX_LAYOUT_RECURSION_DEPTH {
            return Err(self.layout_depth_overflow(node_id, depth));
        }
        if !record {
            let cache_key = MeasureCacheKey::new(node_id, constraints);
            if let Some(cached) = measure_cache.get(&cache_key).copied() {
                return Ok(cached);
            }
        }
        let node = match self.graph_state.node(node_id) {
            Some(node) => node,
            None => return Ok(LayoutSize::ZERO),
        };

        if record {
            constraints_out.insert(node_id, constraints);
        }

        if record {
            if let Some(reused) =
                self.copy_cached_subtree(node_id, origin, constraints, out, constraints_out)?
            {
                return Ok(reused);
            }
        }

        let mut flow_children: Vec<WidgetId> = Vec::new();
        let mut abs_children: Vec<WidgetId> = Vec::new();
        for child_id in self.graph_state.children_of(node_id) {
            let is_absolute = matches!(
                self.graph_state.node(*child_id).map(|n| &n.op),
                Some(LayoutOp::AbsoluteFill)
                    | Some(LayoutOp::Positioned { .. })
                    | Some(LayoutOp::PositionedLengths { .. })
            );
            if is_absolute {
                abs_children.push(*child_id);
            } else {
                flow_children.push(*child_id);
            }
        }
        let rich_text_inline_children = node.rich_text.is_some() && !flow_children.is_empty();

        let mut resolved_style_op = match &node.op {
            LayoutOp::StyledBox {
                style,
                flex_grow,
                flex_shrink,
            } => {
                let mut op = resolve_box_style(style, constraints, self.active_viewport);
                if let LayoutOp::Box {
                    flex_grow: resolved_grow,
                    flex_shrink: resolved_shrink,
                    ..
                } = &mut op
                {
                    *resolved_grow = *flex_grow;
                    *resolved_shrink = *flex_shrink;
                }
                Some(op)
            }
            _ => None,
        };
        if let (
            LayoutOp::StyledBox { style, .. },
            Some(LayoutOp::Box {
                width,
                min_width,
                max_width,
                padding,
                ..
            }),
        ) = (&node.op, &mut resolved_style_op)
        {
            let needs_intrinsic_width = [
                style.width.as_ref(),
                style.min_width.as_ref(),
                style.max_width.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(length_requires_measurement);
            if needs_intrinsic_width {
                let mut min_content = 0.0f32;
                let mut max_content = 0.0f32;
                if let Some((paragraph_min, paragraph_max)) =
                    self.paragraph_intrinsic_widths(node_id)?
                {
                    min_content = paragraph_min;
                    max_content = paragraph_max;
                } else if let (Some(runs), Some(measurer)) = (&node.rich_text, &self.measurer) {
                    min_content = runs
                        .iter()
                        .flat_map(|run| {
                            run.text.split_whitespace().map(move |word| {
                                measurer.measure(word, run.style.font_size, None).0
                                    + run.style.letter_spacing
                                        * word.chars().count().saturating_sub(1) as f32
                            })
                        })
                        .fold(0.0, f32::max);
                    max_content = measurer.layout_rich_text(runs, None).width;
                }
                for child_id in &flow_children {
                    min_content = min_content.max(self.measure_grid_intrinsic_width(
                        *child_id,
                        IntrinsicAxis::Min,
                        constraints.max_h,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        depth + 1,
                    )?);
                    max_content = max_content.max(self.measure_grid_intrinsic_width(
                        *child_id,
                        IntrinsicAxis::Max,
                        constraints.max_h,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        depth + 1,
                    )?);
                }
                let horizontal_padding = padding[0] + padding[1];
                min_content += horizontal_padding;
                max_content =
                    max_content.max(min_content - horizontal_padding) + horizontal_padding;
                let available = if constraints.max_w.is_finite() {
                    constraints.max_w
                } else {
                    max_content
                };
                let resolve = |length: &Option<Length>| {
                    length.as_ref().and_then(|length| {
                        resolve_measured_length(
                            length,
                            available,
                            self.active_viewport,
                            min_content,
                            max_content,
                        )
                    })
                };
                *width = resolve(&style.width);
                *min_width = resolve(&style.min_width);
                *max_width = resolve(&style.max_width);
            }
        }
        let layout_op = resolved_style_op.as_ref().unwrap_or(&node.op);
        let box_alignment = match &node.op {
            LayoutOp::StyledBox { style, .. } => style.alignment,
            // Legacy low-level Box nodes have always stretched an auto-sized
            // child across the parent's cross axis. StyledBox carries an
            // explicit alignment and may opt into start/center/end instead.
            LayoutOp::Box { .. }
                if node.rich_text.is_some()
                    || node.parent_id.is_some_and(|parent_id| {
                        matches!(
                            self.graph_state.node(parent_id).map(|parent| &parent.op),
                            Some(LayoutOp::Flex { .. })
                                | Some(LayoutOp::Align)
                                | Some(LayoutOp::StyledBox { flex_grow: 0.0, .. })
                        )
                    }) =>
            {
                fission_ir::op::BoxAlignment::Start
            }
            LayoutOp::Box { .. } => fission_ir::op::BoxAlignment::Stretch,
            _ => fission_ir::op::BoxAlignment::Start,
        };
        let intrinsic_box_width = match &node.op {
            LayoutOp::StyledBox { style, .. } => style.width.as_ref(),
            _ => None,
        };
        let intrinsic_box_height = match &node.op {
            LayoutOp::StyledBox { style, .. } => style.height.as_ref(),
            _ => None,
        };

        let mut content_size;
        let size = match layout_op {
            LayoutOp::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                aspect_ratio,
                ..
            } => {
                let mut local =
                    constraints.apply_min_max(*min_width, *max_width, *min_height, *max_height);
                local = local.tighten(*width, *height);
                // A measured text node must retain its intrinsic height when
                // its parent supplies a loose cross-axis constraint. Applying
                // that constraint as a tight height makes tooltips and row
                // labels fill the viewport instead of sizing to their lines.
                if node.rich_text.is_some() && height.is_none() {
                    local.min_h = 0.0;
                    local.max_h = f32::INFINITY;
                }
                if let Some(ratio) = aspect_ratio.filter(|r| *r > 0.0) {
                    let mut target_w = *width;
                    let mut target_h = *height;

                    if target_w.is_some() && target_h.is_none() {
                        target_h = target_w.map(|w| w / ratio);
                    } else if target_h.is_some() && target_w.is_none() {
                        target_w = target_h.map(|h| h * ratio);
                    } else if target_w.is_none() && target_h.is_none() {
                        if local.is_width_bounded() || local.is_height_bounded() {
                            let (mut w, mut h) = if local.is_width_bounded() {
                                let w = local.max_w;
                                let h = w / ratio;
                                (w, h)
                            } else {
                                let h = local.max_h;
                                let w = h * ratio;
                                (w, h)
                            };
                            if local.is_width_bounded()
                                && local.is_height_bounded()
                                && h > local.max_h
                            {
                                h = local.max_h;
                                w = h * ratio;
                            }
                            target_w = Some(w);
                            target_h = Some(h);
                        }
                    }

                    if target_w.is_some() || target_h.is_some() {
                        local = local.tighten(target_w, target_h);
                    }
                }
                let mut base_child_constraints = local.deflate(*padding);
                if matches!(intrinsic_box_width, Some(Length::MaxContent)) {
                    base_child_constraints.min_w = 0.0;
                    base_child_constraints.max_w = f32::INFINITY;
                }
                // `fit-content` must measure the child's natural block-axis
                // extent before the result is clamped to the available box.
                // Passing the finite viewport maximum into a column allows
                // stretch-aware descendants to report the whole viewport,
                // making short dialogs and popovers viewport-height.
                if matches!(intrinsic_box_height, Some(Length::FitContent(_))) {
                    base_child_constraints.min_h = 0.0;
                    base_child_constraints.max_h = f32::INFINITY;
                }
                if box_alignment != fission_ir::op::BoxAlignment::Stretch {
                    base_child_constraints.min_w = 0.0;
                    base_child_constraints.min_h = 0.0;
                }
                let mut max_child = LayoutSize::ZERO;
                let mut measured_children: Vec<(WidgetId, BoxConstraints, LayoutSize)> = Vec::new();
                if !rich_text_inline_children {
                    for child_id in &flow_children {
                        let (child_width, child_height, child_max_width, child_max_height) = self
                            .graph_state
                            .node(*child_id)
                            .map(|child| match &child.op {
                                LayoutOp::Box {
                                    width,
                                    height,
                                    max_width,
                                    max_height,
                                    ..
                                } => (*width, *height, *max_width, *max_height),
                                LayoutOp::Scroll {
                                    width,
                                    height,
                                    max_width,
                                    max_height,
                                    ..
                                } => (*width, *height, *max_width, *max_height),
                                LayoutOp::Embed { width, height, .. } => {
                                    (*width, *height, None, None)
                                }
                                LayoutOp::StyledBox { style, .. } => {
                                    let resolved = resolve_box_style(
                                        style,
                                        base_child_constraints,
                                        self.active_viewport,
                                    );
                                    match resolved {
                                        LayoutOp::Box {
                                            width,
                                            height,
                                            max_width,
                                            max_height,
                                            ..
                                        } => (width, height, max_width, max_height),
                                        _ => unreachable!(),
                                    }
                                }
                                _ => (None, None, None, None),
                            })
                            .unwrap_or((None, None, None, None));
                        let mut child_constraints = base_child_constraints;
                        let child_is_align = self
                            .graph_state
                            .node(*child_id)
                            .is_some_and(|child| matches!(&child.op, LayoutOp::Align));
                        // Align intentionally fills a bounded constraint. When it
                        // is the direct child of an auto-sized, non-stretch box,
                        // measure it intrinsically so controls such as Button do
                        // not grow to the full loose width or height supplied by
                        // a flex line. Other children retain the finite maximum
                        // so text wrapping and bounded layout remain intact.
                        if box_alignment != fission_ir::op::BoxAlignment::Stretch && child_is_align
                        {
                            if width.is_none() && local.min_w < local.max_w {
                                child_constraints.max_w = f32::INFINITY;
                            }
                            if height.is_none() && local.min_h < local.max_h {
                                child_constraints.max_h = f32::INFINITY;
                            }
                        }
                        if matches!(intrinsic_box_width, Some(Length::MinContent)) {
                            let intrinsic_width = self.measure_grid_intrinsic_width(
                                *child_id,
                                IntrinsicAxis::Min,
                                base_child_constraints.max_h,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                depth + 1,
                            )?;
                            child_constraints.min_w = intrinsic_width;
                            child_constraints.max_w = intrinsic_width;
                        }
                        let tight_width = child_constraints.min_w == child_constraints.max_w;
                        // Stretch consumes space the box actually owns. A finite maximum on an
                        // auto-sized axis is only a bound: tightening it here makes intrinsic
                        // surfaces such as flyouts expand to the viewport.
                        let stretch_width =
                            tight_width && child_width.is_none() && child_max_width.is_none();
                        if stretch_width {
                            child_constraints.min_w = child_constraints.max_w;
                        } else if tight_width
                            && (child_width.is_some() || child_max_width.is_some())
                        {
                            child_constraints.min_w = 0.0;
                        }
                        let tight_height = child_constraints.min_h == child_constraints.max_h;
                        let stretch_height =
                            tight_height && child_height.is_none() && child_max_height.is_none();
                        if stretch_height {
                            child_constraints.min_h = child_constraints.max_h;
                        } else if tight_height
                            && (child_height.is_some() || child_max_height.is_some())
                        {
                            child_constraints.min_h = 0.0;
                        }
                        let child_size = self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
                        max_child.width = max_child.width.max(child_size.width);
                        max_child.height = max_child.height.max(child_size.height);
                        measured_children.push((*child_id, child_constraints, child_size));
                    }
                }
                let padded = LayoutSize::new(
                    max_child.width + padding[0] + padding[1],
                    max_child.height + padding[2] + padding[3],
                );
                if let LayoutOp::StyledBox { style, .. } = &node.op {
                    let available = if constraints.max_h.is_finite() {
                        constraints.max_h
                    } else {
                        padded.height
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
                                    padded.height,
                                    padded.height,
                                )
                            })
                    };
                    local = local.apply_min_max(
                        None,
                        None,
                        resolve_intrinsic_height(&style.min_height),
                        resolve_intrinsic_height(&style.max_height),
                    );
                    local = local.tighten(None, resolve_intrinsic_height(&style.height));
                }
                let size = local.constrain(padded);
                if record {
                    for (child_id, child_constraints, child_size) in measured_children {
                        let inner_width = (size.width - padding[0] - padding[1]).max(0.0);
                        let inner_height = (size.height - padding[2] - padding[3]).max(0.0);
                        let offset = |available: f32, child: f32| match box_alignment {
                            fission_ir::op::BoxAlignment::Start
                            | fission_ir::op::BoxAlignment::Stretch => 0.0,
                            fission_ir::op::BoxAlignment::Center => {
                                ((available - child) / 2.0).max(0.0)
                            }
                            fission_ir::op::BoxAlignment::End => (available - child).max(0.0),
                        };
                        self.layout_node_constraints(
                            child_id,
                            child_constraints,
                            LayoutPoint::new(
                                origin.x + padding[0] + offset(inner_width, child_size.width),
                                origin.y + padding[2] + offset(inner_height, child_size.height),
                            ),
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                    if !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                }
                content_size = padded;
                size
            }
            LayoutOp::Flex {
                direction,
                wrap,
                padding,
                gap,
                align_items,
                justify_content,
                flex_grow,
                ..
            } => {
                let gap = gap.unwrap_or(0.0);
                let local = constraints.tighten(node.width, node.height);
                let inner = local.deflate(*padding);
                let is_row = matches!(direction, IrFlexDirection::Row);

                let max_main = if is_row { inner.max_w } else { inner.max_h };
                let max_cross = if is_row { inner.max_h } else { inner.max_w };
                let min_main = if is_row { inner.min_w } else { inner.min_h };
                let min_cross = if is_row { inner.min_h } else { inner.min_w };
                let main_bounded = if is_row {
                    inner.is_width_bounded()
                } else {
                    inner.is_height_bounded()
                };
                let cross_bounded = if is_row {
                    inner.is_height_bounded()
                } else {
                    inner.is_width_bounded()
                };

                if matches!(wrap, IrFlexWrap::Wrap | IrFlexWrap::WrapReverse) {
                    let mut lines: Vec<(Vec<(WidgetId, LayoutSize, BoxConstraints)>, f32, f32)> =
                        Vec::new();
                    let mut line_children: Vec<(WidgetId, LayoutSize, BoxConstraints)> = Vec::new();
                    let mut line_main = 0.0f32;
                    let mut line_cross = 0.0f32;
                    let mut max_line_main = 0.0f32;

                    for child_id in &flow_children {
                        let has_explicit_main = self
                            .graph_state
                            .node(*child_id)
                            .is_some_and(|child| has_explicit_main_axis_size(child, is_row));
                        // Measure wrapped children at their intrinsic main-axis size.
                        // Giving every auto-sized child the full line width makes legacy
                        // Box-backed controls (buttons, switches, tags) expand to one
                        // item per line instead of wrapping like CSS flex items.
                        let mut child_constraints = if is_row {
                            BoxConstraints {
                                min_w: 0.0,
                                max_w: if main_bounded && has_explicit_main {
                                    max_main
                                } else {
                                    f32::INFINITY
                                },
                                min_h: 0.0,
                                max_h: max_cross,
                            }
                        } else {
                            BoxConstraints {
                                min_w: 0.0,
                                max_w: max_cross,
                                min_h: 0.0,
                                max_h: if main_bounded && has_explicit_main {
                                    max_main
                                } else {
                                    f32::INFINITY
                                },
                            }
                        };
                        let mut child_size = self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
                        let mut child_main = if is_row {
                            child_size.width
                        } else {
                            child_size.height
                        };
                        if main_bounded && child_main > max_main {
                            if is_row {
                                child_constraints.max_w = max_main;
                            } else {
                                child_constraints.max_h = max_main;
                            }
                            child_size = self.layout_node_constraints(
                                *child_id,
                                child_constraints,
                                LayoutPoint::ZERO,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                false,
                                depth + 1,
                            )?;
                            child_main = if is_row {
                                child_size.width
                            } else {
                                child_size.height
                            };
                        }
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        let next_main = if line_children.is_empty() {
                            child_main
                        } else {
                            line_main + gap + child_main
                        };

                        if main_bounded && !line_children.is_empty() && next_main > max_main {
                            max_line_main = max_line_main.max(line_main);
                            lines.push((line_children, line_main, line_cross));
                            line_children = Vec::new();
                            line_main = 0.0;
                            line_cross = 0.0;
                        }

                        if !line_children.is_empty() {
                            line_main += gap;
                        }
                        line_main += child_main;
                        line_cross = line_cross.max(child_cross);
                        line_children.push((*child_id, child_size, child_constraints));
                    }

                    if !line_children.is_empty() {
                        max_line_main = max_line_main.max(line_main);
                        lines.push((line_children, line_main, line_cross));
                    }

                    let mut container_main = if main_bounded && *flex_grow > 0.0 {
                        max_main
                    } else {
                        max_line_main
                    };
                    container_main = container_main.max(min_main);
                    let total_lines_cross: f32 =
                        lines.iter().map(|(_, _, cross)| *cross).sum::<f32>()
                            + gap * lines.len().saturating_sub(1) as f32;
                    let container_cross = total_lines_cross.max(min_cross);
                    let size = if is_row {
                        local.constrain(LayoutSize::new(
                            container_main + padding[0] + padding[1],
                            container_cross + padding[2] + padding[3],
                        ))
                    } else {
                        local.constrain(LayoutSize::new(
                            container_cross + padding[0] + padding[1],
                            container_main + padding[2] + padding[3],
                        ))
                    };

                    let inner_main = if is_row {
                        size.width - padding[0] - padding[1]
                    } else {
                        size.height - padding[2] - padding[3]
                    };
                    let inner_cross = if is_row {
                        size.height - padding[2] - padding[3]
                    } else {
                        size.width - padding[0] - padding[1]
                    };

                    let mut ordered_lines = lines;
                    if matches!(wrap, IrFlexWrap::WrapReverse) {
                        ordered_lines.reverse();
                    }

                    let mut line_cursor = if matches!(wrap, IrFlexWrap::WrapReverse) {
                        (inner_cross - total_lines_cross).max(0.0)
                    } else {
                        0.0
                    };

                    for (line_children, line_main, line_cross) in ordered_lines {
                        let remaining_space = (inner_main - line_main).max(0.0);
                        let mut extra_gap = 0.0;
                        let mut offset_main = 0.0;
                        match justify_content {
                            fission_ir::op::JustifyContent::Start => {}
                            fission_ir::op::JustifyContent::End => offset_main = remaining_space,
                            fission_ir::op::JustifyContent::Center => {
                                offset_main = remaining_space / 2.0
                            }
                            fission_ir::op::JustifyContent::SpaceBetween => {
                                if line_children.len() > 1 {
                                    extra_gap =
                                        remaining_space / (line_children.len() as f32 - 1.0);
                                }
                            }
                            fission_ir::op::JustifyContent::SpaceAround => {
                                if !line_children.is_empty() {
                                    extra_gap = remaining_space / line_children.len() as f32;
                                    offset_main = extra_gap / 2.0;
                                }
                            }
                            fission_ir::op::JustifyContent::SpaceEvenly => {
                                if !line_children.is_empty() {
                                    extra_gap =
                                        remaining_space / (line_children.len() as f32 + 1.0);
                                    offset_main = extra_gap;
                                }
                            }
                        }

                        let mut cursor = offset_main;
                        for (child_id, child_size, mut child_constraints) in line_children {
                            let child_main = if is_row {
                                child_size.width
                            } else {
                                child_size.height
                            };
                            let child_cross = if is_row {
                                child_size.height
                            } else {
                                child_size.width
                            };
                            let has_explicit_cross = self
                                .graph_state
                                .node(child_id)
                                .is_some_and(|child| has_explicit_cross_axis_size(child, is_row));
                            if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                && !has_explicit_cross
                            {
                                if is_row {
                                    child_constraints.min_h = line_cross;
                                    child_constraints.max_h = line_cross;
                                } else {
                                    child_constraints.min_w = line_cross;
                                    child_constraints.max_w = line_cross;
                                }
                            }
                            let cross_offset = match align_items {
                                fission_ir::op::AlignItems::Start
                                | fission_ir::op::AlignItems::Stretch => 0.0,
                                fission_ir::op::AlignItems::End => {
                                    (line_cross - child_cross).max(0.0)
                                }
                                fission_ir::op::AlignItems::Center => {
                                    ((line_cross - child_cross) / 2.0).max(0.0)
                                }
                                fission_ir::op::AlignItems::Baseline => 0.0,
                            };
                            let child_origin = if is_row {
                                LayoutPoint::new(
                                    origin.x + padding[0] + cursor,
                                    origin.y + padding[2] + line_cursor + cross_offset,
                                )
                            } else {
                                LayoutPoint::new(
                                    origin.x + padding[0] + line_cursor + cross_offset,
                                    origin.y + padding[2] + cursor,
                                )
                            };
                            self.layout_node_constraints(
                                child_id,
                                child_constraints,
                                child_origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                            cursor += child_main + gap + extra_gap;
                        }

                        line_cursor += line_cross + gap;
                    }

                    if record && !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                    content_size = size;
                    size
                } else {
                    struct FlexChildEntry {
                        id: WidgetId,
                        flex: f32,
                        size: LayoutSize,
                        constraints: BoxConstraints,
                        is_flex: bool,
                    }
                    let mut measured: Vec<FlexChildEntry> = Vec::new();
                    let mut total_flex = 0.0f32;
                    let mut nonflex_main = 0.0f32;
                    let mut max_child_cross = 0.0f32;
                    let treat_flex_as_nonflex = !main_bounded;

                    for child_id in &flow_children {
                        let child = match self.graph_state.node(*child_id) {
                            Some(child) => child,
                            None => continue,
                        };
                        let has_explicit_cross = has_explicit_cross_axis_size(child, is_row);
                        let has_explicit_main = has_explicit_main_axis_size(child, is_row);
                        let flex = child.flex_grow;
                        if flex > 0.0 && !treat_flex_as_nonflex {
                            total_flex += flex;
                            measured.push(FlexChildEntry {
                                id: *child_id,
                                flex,
                                size: LayoutSize::ZERO,
                                constraints: BoxConstraints::loose(0.0, 0.0),
                                is_flex: true,
                            });
                            continue;
                        }
                        let child_constraints = if is_row {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                    && !has_explicit_cross
                                    && child.rich_text.is_none()
                                    && !matches!(
                                        child.op,
                                        LayoutOp::Box {
                                            width: None,
                                            height: None,
                                            ..
                                        }
                                    )
                                {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: if main_bounded && has_explicit_main {
                                            max_main
                                        } else {
                                            f32::INFINITY
                                        },
                                        min_h: max_cross,
                                        max_h: max_cross,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: if main_bounded && has_explicit_main {
                                            max_main
                                        } else {
                                            f32::INFINITY
                                        },
                                        min_h: 0.0,
                                        max_h: max_cross,
                                    }
                                };
                            cross
                        } else {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                    && !has_explicit_cross
                                    && child.rich_text.is_none()
                                    && !matches!(
                                        child.op,
                                        LayoutOp::Box {
                                            width: None,
                                            height: None,
                                            ..
                                        }
                                    )
                                {
                                    BoxConstraints {
                                        min_w: max_cross,
                                        max_w: max_cross,
                                        min_h: 0.0,
                                        max_h: if main_bounded && has_explicit_main {
                                            max_main
                                        } else {
                                            f32::INFINITY
                                        },
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: max_cross,
                                        min_h: 0.0,
                                        max_h: if main_bounded && has_explicit_main {
                                            max_main
                                        } else {
                                            f32::INFINITY
                                        },
                                    }
                                };
                            cross
                        };
                        let child_size = self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
                        let child_main = if is_row {
                            child_size.width
                        } else {
                            child_size.height
                        };
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        nonflex_main += child_main;
                        max_child_cross = max_child_cross.max(child_cross);
                        measured.push(FlexChildEntry {
                            id: *child_id,
                            flex,
                            size: child_size,
                            constraints: child_constraints,
                            is_flex: false,
                        });
                    }

                    let gap_total = gap * flow_children.len().saturating_sub(1) as f32;
                    let remaining = if main_bounded {
                        (max_main - nonflex_main - gap_total).max(0.0)
                    } else {
                        0.0
                    };

                    for entry in measured.iter_mut().filter(|e| e.is_flex) {
                        let flex = entry.flex;
                        let has_explicit_cross = self
                            .graph_state
                            .node(entry.id)
                            .is_some_and(|child| has_explicit_cross_axis_size(child, is_row));
                        let allocated = if main_bounded && total_flex > 0.0 {
                            remaining * (flex / total_flex)
                        } else {
                            0.0
                        };
                        let child_constraints = if is_row {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                    && !has_explicit_cross
                                {
                                    BoxConstraints {
                                        min_w: allocated,
                                        max_w: allocated,
                                        min_h: max_cross,
                                        max_h: max_cross,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: allocated,
                                        max_w: allocated,
                                        min_h: 0.0,
                                        max_h: max_cross,
                                    }
                                };
                            cross
                        } else {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                    && !has_explicit_cross
                                {
                                    BoxConstraints {
                                        min_w: max_cross,
                                        max_w: max_cross,
                                        min_h: allocated,
                                        max_h: allocated,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: max_cross,
                                        min_h: allocated,
                                        max_h: allocated,
                                    }
                                };
                            cross
                        };
                        let child_size = self.layout_node_constraints(
                            entry.id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        max_child_cross = max_child_cross.max(child_cross);
                        entry.size = child_size;
                        entry.constraints = child_constraints;
                    }

                    let final_children_main: f32 = measured
                        .iter()
                        .map(|entry| {
                            if is_row {
                                entry.size.width
                            } else {
                                entry.size.height
                            }
                        })
                        .sum();

                    let mut container_main = if main_bounded && *flex_grow > 0.0 {
                        max_main
                    } else {
                        final_children_main + gap_total
                    };
                    container_main = container_main.max(min_main);

                    if main_bounded && final_children_main + gap_total > max_main {
                        // SHRINK logic
                        let mut total_shrink_scaled = 0.0f32;
                        for entry in &measured {
                            let Some(child) = self.graph_state.node(entry.id) else {
                                continue;
                            };
                            let main_size = if is_row {
                                entry.size.width
                            } else {
                                entry.size.height
                            };
                            total_shrink_scaled += main_size * child.flex_shrink;
                        }

                        if total_shrink_scaled > 0.0 {
                            let overflow = (final_children_main + gap_total) - max_main;
                            for entry in &mut measured {
                                let Some(child) = self.graph_state.node(entry.id) else {
                                    continue;
                                };
                                let main_size = if is_row {
                                    entry.size.width
                                } else {
                                    entry.size.height
                                };
                                let shrink_amount = (main_size * child.flex_shrink
                                    / total_shrink_scaled)
                                    * overflow;
                                // Don't shrink below a reasonable minimum. Items with
                                // flex_shrink > 0 can shrink but not to zero - preserve at
                                // least a small fraction of their natural size.
                                let floor = if child.flex_shrink > 0.0 {
                                    // Check for explicit min/fixed dimension
                                    let explicit_min = match &child.op {
                                        LayoutOp::Box {
                                            min_width,
                                            min_height,
                                            height,
                                            width,
                                            ..
                                        } => {
                                            if is_row {
                                                min_width.or(*width).unwrap_or(0.0)
                                            } else {
                                                min_height.or(*height).unwrap_or(0.0)
                                            }
                                        }
                                        _ => 0.0,
                                    };
                                    explicit_min
                                } else {
                                    main_size // flex_shrink == 0 means don't shrink at all
                                };
                                let new_main = (main_size - shrink_amount).max(floor);

                                let mut child_constraints = entry.constraints;
                                if is_row {
                                    child_constraints.min_w = new_main;
                                    child_constraints.max_w = new_main;
                                } else {
                                    child_constraints.min_h = new_main;
                                    child_constraints.max_h = new_main;
                                }
                                let new_size = self.layout_node_constraints(
                                    entry.id,
                                    child_constraints,
                                    LayoutPoint::ZERO,
                                    out,
                                    constraints_out,
                                    measure_cache,
                                    scroll_source,
                                    false,
                                    depth + 1,
                                )?;
                                entry.size = new_size;
                                entry.constraints = child_constraints;
                            }
                        }
                    }

                    let container_cross = max_child_cross.max(min_cross);
                    let size = if is_row {
                        local.constrain(LayoutSize::new(
                            container_main + padding[0] + padding[1],
                            container_cross + padding[2] + padding[3],
                        ))
                    } else {
                        local.constrain(LayoutSize::new(
                            container_cross + padding[0] + padding[1],
                            container_main + padding[2] + padding[3],
                        ))
                    };

                    let inner_main = if is_row {
                        size.width - padding[0] - padding[1]
                    } else {
                        size.height - padding[2] - padding[3]
                    };
                    let inner_cross = if is_row {
                        size.height - padding[2] - padding[3]
                    } else {
                        size.width - padding[0] - padding[1]
                    };

                    let final_children_main: f32 = measured
                        .iter()
                        .map(|entry| {
                            if is_row {
                                entry.size.width
                            } else {
                                entry.size.height
                            }
                        })
                        .sum();

                    let remaining_space = (inner_main - final_children_main - gap_total).max(0.0);
                    let mut extra_gap = 0.0;
                    let mut offset_main = 0.0;
                    match justify_content {
                        fission_ir::op::JustifyContent::Start => {}
                        fission_ir::op::JustifyContent::End => offset_main = remaining_space,
                        fission_ir::op::JustifyContent::Center => {
                            offset_main = remaining_space / 2.0
                        }
                        fission_ir::op::JustifyContent::SpaceBetween => {
                            if measured.len() > 1 {
                                extra_gap = remaining_space / (measured.len() as f32 - 1.0);
                            }
                        }
                        fission_ir::op::JustifyContent::SpaceAround => {
                            if !measured.is_empty() {
                                extra_gap = remaining_space / measured.len() as f32;
                                offset_main = extra_gap / 2.0;
                            }
                        }
                        fission_ir::op::JustifyContent::SpaceEvenly => {
                            if !measured.is_empty() {
                                extra_gap = remaining_space / (measured.len() as f32 + 1.0);
                                offset_main = extra_gap;
                            }
                        }
                    }

                    let mut cursor = offset_main;
                    for entry in measured {
                        let child_main = if is_row {
                            entry.size.width
                        } else {
                            entry.size.height
                        };
                        let child_cross = if is_row {
                            entry.size.height
                        } else {
                            entry.size.width
                        };
                        let cross_offset = match align_items {
                            fission_ir::op::AlignItems::Start
                            | fission_ir::op::AlignItems::Stretch => 0.0,
                            fission_ir::op::AlignItems::End => (inner_cross - child_cross).max(0.0),
                            fission_ir::op::AlignItems::Center => {
                                ((inner_cross - child_cross) / 2.0).max(0.0)
                            }
                            fission_ir::op::AlignItems::Baseline => 0.0,
                        };
                        let child_origin = if is_row {
                            LayoutPoint::new(
                                origin.x + padding[0] + cursor,
                                origin.y + padding[2] + cross_offset,
                            )
                        } else {
                            LayoutPoint::new(
                                origin.x + padding[0] + cross_offset,
                                origin.y + padding[2] + cursor,
                            )
                        };

                        let mut child_constraints = entry.constraints;
                        if matches!(align_items, fission_ir::op::AlignItems::Stretch) {
                            // Only stretch children that don't have an explicit cross-axis size.
                            let child_node = self.graph_state.node(entry.id);
                            let has_explicit_cross = child_node
                                .is_some_and(|node| has_explicit_cross_axis_size(node, is_row));
                            // Text owns its measured height/width; stretching the
                            // text layout node would turn a line into the full
                            // row height and distort vertical centering.
                            let is_measured_text = child_node.is_some_and(|node| {
                                node.rich_text.is_some()
                                    || matches!(
                                        node.op,
                                        LayoutOp::Box {
                                            width: None,
                                            height: None,
                                            ..
                                        }
                                    )
                            });
                            if !has_explicit_cross && !is_measured_text {
                                if is_row {
                                    child_constraints.min_h = inner_cross;
                                    child_constraints.max_h = inner_cross;
                                } else {
                                    child_constraints.min_w = inner_cross;
                                    child_constraints.max_w = inner_cross;
                                }
                            }
                        }

                        self.layout_node_constraints(
                            entry.id,
                            child_constraints,
                            child_origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                        cursor += child_main + gap + extra_gap;
                    }

                    if record && !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                    content_size = size;
                    size
                }
            }
            LayoutOp::Grid {
                columns,
                rows,
                column_gap,
                row_gap,
                padding,
            } => {
                let size = self.layout_grid(
                    columns,
                    rows,
                    column_gap,
                    row_gap,
                    padding,
                    constraints,
                    origin,
                    &flow_children,
                    &abs_children,
                    out,
                    constraints_out,
                    measure_cache,
                    scroll_source,
                    record,
                    depth,
                )?;
                content_size = size;
                size
            }
            LayoutOp::GridItem { .. } => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        constraints,
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Responsive { query, cases } => {
                let query_width = match query {
                    fission_ir::op::ResponsiveQuery::Viewport => self.active_viewport.width,
                    fission_ir::op::ResponsiveQuery::Container => {
                        if constraints.is_width_bounded() {
                            constraints.max_w
                        } else {
                            self.active_viewport.width
                        }
                    }
                };
                let selected_index = cases
                    .iter()
                    .enumerate()
                    .find_map(|(index, condition)| condition.matches(query_width).then_some(index))
                    .unwrap_or(cases.len());
                let child_size = node
                    .children_ids
                    .get(selected_index)
                    .map(|child_id| {
                        self.layout_node_constraints(
                            *child_id,
                            constraints,
                            origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )
                    })
                    .transpose()?
                    .unwrap_or(LayoutSize::ZERO);
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Scroll {
                direction,
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                ..
            } => {
                let mut local =
                    constraints.apply_min_max(*min_width, *max_width, *min_height, *max_height);
                local = local.tighten(*width, *height);
                let is_horizontal = matches!(direction, IrFlexDirection::Row);
                let mut child_constraints = local.deflate(*padding);
                if is_horizontal {
                    child_constraints.min_w = 0.0;
                    child_constraints.max_w = f32::INFINITY;
                } else {
                    child_constraints.min_h = 0.0;
                    child_constraints.max_h = f32::INFINITY;
                }
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = flow_children.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                }
                let size = local.constrain(LayoutSize::new(
                    child_size.width + padding[0] + padding[1],
                    child_size.height + padding[2] + padding[3],
                ));
                if record {
                    if let Some(child_id) = flow_children.first() {
                        self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::new(origin.x + padding[0], origin.y + padding[2]),
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                    if !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                }
                content_size = child_size;
                size
            }
            LayoutOp::Align => {
                let child_constraints = BoxConstraints::loose(constraints.max_w, constraints.max_h);
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = flow_children.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(
                        if constraints.is_width_bounded() {
                            constraints.max_w
                        } else {
                            child_size.width
                        },
                        if constraints.is_height_bounded() {
                            constraints.max_h
                        } else {
                            child_size.height
                        },
                    ))
                } else {
                    child_size
                };
                if let Some(child_id) = flow_children.first() {
                    let dx = ((size.width - child_size.width) / 2.0).max(0.0);
                    let dy = ((size.height - child_size.height) / 2.0).max(0.0);
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + dx, origin.y + dy),
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(
                            child_id,
                            abs_constraints,
                            origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }
                content_size = child_size;
                size
            }
            LayoutOp::ZStack => {
                let mut max_child = LayoutSize::ZERO;
                for child_id in &flow_children {
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::loose(constraints.max_w, constraints.max_h),
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                    max_child.width = max_child.width.max(child_size.width);
                    max_child.height = max_child.height.max(child_size.height);
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(
                        if constraints.is_width_bounded() {
                            constraints.max_w
                        } else {
                            max_child.width
                        },
                        if constraints.is_height_bounded() {
                            constraints.max_h
                        } else {
                            max_child.height
                        },
                    ))
                } else {
                    max_child
                };
                for child_id in &flow_children {
                    let child_constraints = BoxConstraints::loose(size.width, size.height);
                    let child_origin = LayoutPoint::new(origin.x, origin.y);
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        child_origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(
                            child_id,
                            abs_constraints,
                            origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }
                content_size = size;
                size
            }
            LayoutOp::Positioned {
                top,
                left,
                bottom,
                right,
                width,
                height,
            } => {
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
                let mut child_constraints = BoxConstraints::loose(size.width, size.height);
                if let (Some(l), Some(r)) = (left, right) {
                    let w = (size.width - l - r).max(0.0);
                    child_constraints = child_constraints.tighten(Some(w), None);
                }
                if let (Some(t), Some(b)) = (top, bottom) {
                    let h = (size.height - t - b).max(0.0);
                    child_constraints = child_constraints.tighten(None, Some(h));
                }
                child_constraints = child_constraints.tighten(*width, *height);
                if let Some(child_id) = node.children_ids.first() {
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                    let x = left.unwrap_or_else(|| {
                        right
                            .map(|r| (size.width - r - child_size.width).max(0.0))
                            .unwrap_or(0.0)
                    });
                    let y = top.unwrap_or_else(|| {
                        bottom
                            .map(|b| (size.height - b - child_size.height).max(0.0))
                            .unwrap_or(0.0)
                    });
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + x, origin.y + y),
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = size;
                size
            }
            LayoutOp::PositionedLengths {
                top,
                left,
                bottom,
                right,
                width,
                height,
            } => {
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
                let resolve_horizontal = |length: &Option<Length>| {
                    length
                        .as_ref()
                        .and_then(|length| resolve_length(length, size.width, self.active_viewport))
                };
                let resolve_vertical = |length: &Option<Length>| {
                    length.as_ref().and_then(|length| {
                        resolve_length(length, size.height, self.active_viewport)
                    })
                };
                let left = resolve_horizontal(left);
                let top = resolve_vertical(top);
                let right = resolve_horizontal(right);
                let bottom = resolve_vertical(bottom);
                let width = resolve_horizontal(width);
                let height = resolve_vertical(height);
                let mut child_constraints = BoxConstraints::loose(size.width, size.height);
                if let (Some(left), Some(right)) = (left, right) {
                    child_constraints =
                        child_constraints.tighten(Some((size.width - left - right).max(0.0)), None);
                }
                if let (Some(top), Some(bottom)) = (top, bottom) {
                    child_constraints = child_constraints
                        .tighten(None, Some((size.height - top - bottom).max(0.0)));
                }
                child_constraints = child_constraints.tighten(width, height);
                if let Some(child_id) = node.children_ids.first() {
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                    let x = left.unwrap_or_else(|| {
                        right
                            .map(|right| (size.width - right - child_size.width).max(0.0))
                            .unwrap_or(0.0)
                    });
                    let y = top.unwrap_or_else(|| {
                        bottom
                            .map(|bottom| (size.height - bottom - child_size.height).max(0.0))
                            .unwrap_or(0.0)
                    });
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + x, origin.y + y),
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = size;
                size
            }
            LayoutOp::Embed { width, height, .. } => {
                let local = constraints.tighten(*width, *height);
                let w = if local.is_width_bounded() {
                    local.max_w
                } else {
                    local.min_w
                };
                let h = if local.is_height_bounded() {
                    local.max_h
                } else {
                    local.min_h
                };
                let size = local.constrain(LayoutSize::new(w, h));
                content_size = size;
                size
            }
            LayoutOp::AbsoluteFill => {
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
                for child_id in self.graph_state.children_of(node_id) {
                    self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::tight(size),
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = size;
                size
            }
            LayoutOp::Spotlight { .. } => {
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
                for child_id in self.graph_state.children_of(node_id) {
                    self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::tight(LayoutSize::ZERO),
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = size;
                size
            }
            LayoutOp::Transform { .. } | LayoutOp::Clip { .. } => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        constraints,
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Flyout { anchor, content: _ } => {
                let loose = BoxConstraints::loose(
                    if constraints.is_width_bounded() {
                        constraints.max_w
                    } else {
                        f32::INFINITY
                    },
                    if constraints.is_height_bounded() {
                        constraints.max_h
                    } else {
                        f32::INFINITY
                    },
                );
                let mut child_size = LayoutSize::ZERO;
                for child_id in self.graph_state.children_of(node_id) {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        loose,
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                }
                if record {
                    let anchor_rect = out.get(anchor).map(|g| g.rect);
                    let place_x = anchor_rect.map(|r| r.x()).unwrap_or(origin.x);
                    let place_y = anchor_rect.map(|r| r.y() + r.height()).unwrap_or(origin.y);
                    for child_id in self.graph_state.children_of(node_id) {
                        self.layout_node_constraints(
                            *child_id,
                            loose,
                            LayoutPoint::new(place_x, place_y),
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }
                content_size = child_size;
                child_size
            }
            LayoutOp::StyledBox { .. } => unreachable!("styled boxes are resolved before layout"),
        };

        if let Some(result) = self.layout_rich_text_content(
            node_id,
            node,
            layout_op,
            constraints,
            origin,
            &flow_children,
            rich_text_inline_children,
            &mut content_size,
            out,
            constraints_out,
            measure_cache,
            scroll_source,
            record,
            depth,
        )? {
            return Ok(result);
        }

        let result = self.record_geometry(node_id, origin, size, content_size, out, record);
        if !record {
            measure_cache.insert(MeasureCacheKey::new(node_id, constraints), result);
        }
        Ok(result)
    }
}
