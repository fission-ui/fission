use anyhow::Result;
use fission_ir::{GridPlacement, GridTrack, LayoutOp, WidgetId};
use std::collections::{HashMap, HashSet};

use super::graph::MeasureCacheKey;
use super::LayoutEngine;
use crate::grid_tracks::{
    distribute_deficit, distribute_flex, expand_tracks, IntrinsicAxis, TrackSizing,
};
use crate::{BoxConstraints, LayoutNodeGeometry, LayoutPoint, LayoutSize, ScrollDataSource};

impl LayoutEngine {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn layout_grid(
        &self,
        columns: &[GridTrack],
        rows: &[GridTrack],
        column_gap: &Option<f32>,
        row_gap: &Option<f32>,
        padding: &[f32; 4],
        constraints: BoxConstraints,
        origin: LayoutPoint,
        flow_children: &[WidgetId],
        abs_children: &[WidgetId],
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<WidgetId, BoxConstraints>,
        measure_cache: &mut HashMap<MeasureCacheKey, LayoutSize>,
        scroll_source: &impl ScrollDataSource,
        record: bool,
        depth: usize,
    ) -> Result<LayoutSize> {
        let gap_x = column_gap.unwrap_or(0.0);
        let gap_y = row_gap.unwrap_or(0.0);
        let inner = constraints.deflate(*padding);
        let bounded_w = inner.is_width_bounded();
        let bounded_h = inner.is_height_bounded();
        let child_count = flow_children.len();
        let available_w = bounded_w.then_some(inner.max_w);
        let available_h = bounded_h.then_some(inner.max_h);
        let mut expanded_columns = expand_tracks(columns, available_w, gap_x, child_count);
        if expanded_columns.is_empty() {
            expanded_columns.push(GridTrack::Auto);
        }
        let mut col_count = expanded_columns.len();

        #[derive(Clone, Copy)]
        struct GridCell {
            id: WidgetId,
            row: usize,
            col: usize,
            row_span: usize,
            col_span: usize,
        }

        let mut cell_assignments: Vec<GridCell> = Vec::new();
        let mut auto_row = 0;
        let mut auto_col = 0;
        let mut occupied = HashSet::<(usize, usize)>::new();

        for child_id in flow_children {
            let Some(child) = self.graph_state.node(*child_id) else {
                continue;
            };
            let (row_start, row_end, col_start, col_end) = if let LayoutOp::GridItem {
                row_start,
                row_end,
                col_start,
                col_end,
                ..
            } = &child.op
            {
                (*row_start, *row_end, *col_start, *col_end)
            } else {
                (
                    GridPlacement::Auto,
                    GridPlacement::Auto,
                    GridPlacement::Auto,
                    GridPlacement::Auto,
                )
            };
            let explicit_row = match row_start {
                GridPlacement::Line(line) => Some(line.max(1) as usize - 1),
                _ => None,
            };
            let explicit_col = match col_start {
                GridPlacement::Line(line) => Some(line.max(1) as usize - 1),
                _ => None,
            };
            let row_span = match row_end {
                GridPlacement::Span(span) => usize::from(span).max(1),
                GridPlacement::Line(line) => {
                    let end = line.max(1) as usize - 1;
                    end.saturating_sub(explicit_row.unwrap_or_default()).max(1)
                }
                GridPlacement::Auto => 1,
            };
            let col_span = match col_end {
                GridPlacement::Span(span) => usize::from(span).max(1),
                GridPlacement::Line(line) => {
                    let end = line.max(1) as usize - 1;
                    end.saturating_sub(explicit_col.unwrap_or_default()).max(1)
                }
                GridPlacement::Auto => 1,
            };
            let fits = |row: usize, col: usize, occupied: &HashSet<(usize, usize)>| {
                (row..row + row_span)
                    .all(|row| (col..col + col_span).all(|col| !occupied.contains(&(row, col))))
            };
            let (row, col) = match (explicit_row, explicit_col) {
                (Some(row), Some(col)) => (row, col),
                (Some(row), None) => {
                    let mut col = 0;
                    while !fits(row, col, &occupied) {
                        col += 1;
                    }
                    (row, col)
                }
                (None, Some(col)) => {
                    let mut row = 0;
                    while !fits(row, col, &occupied) {
                        row += 1;
                    }
                    (row, col)
                }
                (None, None) => {
                    while (col_span <= col_count && auto_col + col_span > col_count)
                        || !fits(auto_row, auto_col, &occupied)
                    {
                        auto_col += 1;
                        if auto_col >= col_count {
                            auto_col = 0;
                            auto_row += 1;
                        }
                    }
                    let placement = (auto_row, auto_col);
                    if col_span >= col_count {
                        auto_col = 0;
                        auto_row += 1;
                    } else {
                        auto_col += col_span;
                        if auto_col >= col_count {
                            auto_col = 0;
                            auto_row += 1;
                        }
                    }
                    placement
                }
            };
            for occupied_row in row..row + row_span {
                for occupied_col in col..col + col_span {
                    occupied.insert((occupied_row, occupied_col));
                }
            }
            cell_assignments.push(GridCell {
                id: *child_id,
                row,
                col,
                row_span,
                col_span,
            });
        }

        let required_columns = cell_assignments
            .iter()
            .map(|cell| cell.col + cell.col_span)
            .max()
            .unwrap_or(1);
        if required_columns > col_count {
            expanded_columns.resize(required_columns, GridTrack::Auto);
            col_count = expanded_columns.len();
        }

        let mut column_sizing = expanded_columns
            .iter()
            .map(|track| TrackSizing::from_track(track, available_w))
            .collect::<Vec<_>>();

        for cell in &cell_assignments {
            let intrinsic = column_sizing[cell.col..cell.col + cell.col_span]
                .iter()
                .filter_map(|track| track.intrinsic)
                .fold(None, |current, axis| match (current, axis) {
                    (Some(IntrinsicAxis::Max), _) | (_, IntrinsicAxis::Max) => {
                        Some(IntrinsicAxis::Max)
                    }
                    _ => Some(IntrinsicAxis::Min),
                });
            let Some(intrinsic) = intrinsic else {
                continue;
            };
            let width = self.measure_grid_intrinsic_width(
                cell.id,
                intrinsic,
                inner.max_h,
                out,
                constraints_out,
                measure_cache,
                scroll_source,
                depth + 1,
            )?;
            distribute_deficit(
                &mut column_sizing,
                cell.col,
                cell.col_span,
                (width - gap_x * cell.col_span.saturating_sub(1) as f32).max(0.0),
            );
        }
        if let Some(available_w) = available_w {
            distribute_flex(&mut column_sizing, available_w, gap_x);
        }
        let col_widths = column_sizing
            .iter()
            .map(|track| track.base)
            .collect::<Vec<_>>();

        let minimum_rows = cell_assignments
            .iter()
            .map(|cell| cell.row + cell.row_span)
            .max()
            .unwrap_or_else(|| (child_count + col_count - 1) / col_count)
            .max(1);
        let mut expanded_rows = expand_tracks(rows, available_h, gap_y, minimum_rows);
        if expanded_rows.is_empty() {
            expanded_rows.resize(minimum_rows, GridTrack::Auto);
        } else if expanded_rows.len() < minimum_rows {
            expanded_rows.resize(minimum_rows, GridTrack::Auto);
        }
        let mut row_sizing = expanded_rows
            .iter()
            .map(|track| TrackSizing::from_track(track, available_h))
            .collect::<Vec<_>>();

        for cell in &cell_assignments {
            if cell.row >= row_sizing.len() || cell.col >= col_widths.len() {
                continue;
            }
            let col_end = (cell.col + cell.col_span).min(col_widths.len());
            let cell_w = col_widths[cell.col..col_end].iter().sum::<f32>()
                + gap_x * col_end.saturating_sub(cell.col + 1) as f32;
            let cell_constraints = BoxConstraints {
                min_w: 0.0,
                max_w: cell_w,
                min_h: 0.0,
                max_h: f32::INFINITY,
            };
            let child_size = self.layout_node_constraints(
                cell.id,
                cell_constraints,
                LayoutPoint::ZERO,
                out,
                constraints_out,
                measure_cache,
                scroll_source,
                false,
                depth + 1,
            )?;
            distribute_deficit(
                &mut row_sizing,
                cell.row,
                cell.row_span,
                (child_size.height - gap_y * cell.row_span.saturating_sub(1) as f32).max(0.0),
            );
        }
        if let Some(available_h) = available_h {
            distribute_flex(&mut row_sizing, available_h, gap_y);
        }
        let row_heights = row_sizing
            .iter()
            .map(|track| track.base)
            .collect::<Vec<_>>();

        let grid_w: f32 =
            col_widths.iter().sum::<f32>() + gap_x * (col_count.saturating_sub(1) as f32);
        let grid_h: f32 =
            row_heights.iter().sum::<f32>() + gap_y * (row_heights.len().saturating_sub(1) as f32);
        let size = constraints.constrain(LayoutSize::new(
            grid_w + padding[0] + padding[1],
            grid_h + padding[2] + padding[3],
        ));

        if record {
            let padding_origin_x = origin.x + padding[0];
            let padding_origin_y = origin.y + padding[2];
            for cell in &cell_assignments {
                if cell.row >= row_heights.len() || cell.col >= col_widths.len() {
                    continue;
                }
                let cell_x = padding_origin_x
                    + col_widths[..cell.col].iter().sum::<f32>()
                    + gap_x * cell.col as f32;
                let cell_y = padding_origin_y
                    + row_heights[..cell.row].iter().sum::<f32>()
                    + gap_y * cell.row as f32;
                let col_end = (cell.col + cell.col_span).min(col_widths.len());
                let row_end = (cell.row + cell.row_span).min(row_heights.len());
                let cell_w = col_widths[cell.col..col_end].iter().sum::<f32>()
                    + gap_x * col_end.saturating_sub(cell.col + 1) as f32;
                let cell_h = row_heights[cell.row..row_end].iter().sum::<f32>()
                    + gap_y * row_end.saturating_sub(cell.row + 1) as f32;
                let child_constraints = BoxConstraints {
                    min_w: cell_w,
                    max_w: cell_w,
                    min_h: cell_h,
                    max_h: cell_h,
                };
                self.layout_node_constraints(
                    cell.id,
                    child_constraints,
                    LayoutPoint::new(cell_x, cell_y),
                    out,
                    constraints_out,
                    measure_cache,
                    scroll_source,
                    record,
                    depth + 1,
                )?;
            }
        }

        if record && !abs_children.is_empty() {
            let abs_constraints = BoxConstraints::loose(size.width, size.height);
            for child_id in abs_children {
                self.layout_node_constraints(
                    *child_id,
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
        Ok(size)
    }
}
