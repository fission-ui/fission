use crate::components::capability_cell::CapabilityCell;
use crate::components::ui::ResponsiveGrid;
use crate::model::{CapabilityLine, FieldInspectorState};
use fission::prelude::*;

const CAPABILITY_MIN_WIDTH: f32 = 280.0;

fission_component! {
    pub CapabilityGrid<FieldInspectorState> { lines: Vec<CapabilityLine> }
    |grid, _state, _cx| {
        let children = grid.lines.iter().cloned().map(|line| CapabilityCell::new(line).into()).collect();
    } => ResponsiveGrid(children, CAPABILITY_MIN_WIDTH)
}
