use crate::components::capability_cell::CapabilityCell;
use crate::components::ui::ResponsiveGrid;
use crate::model::CapabilityLine;
use fission::prelude::*;

const CAPABILITY_MIN_WIDTH: f32 = 280.0;

pub struct CapabilityGrid {
    pub lines: Vec<CapabilityLine>,
}

impl From<CapabilityGrid> for Widget {
    fn from(grid: CapabilityGrid) -> Self {
        ResponsiveGrid::new(
            grid.lines
                .into_iter()
                .map(|line| CapabilityCell { line }.into())
                .collect(),
        )
        .item_min_width(CAPABILITY_MIN_WIDTH)
        .into()
    }
}
