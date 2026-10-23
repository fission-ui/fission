use crate::components::overview_heading_summary::OverviewHeadingSummary;
use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! { pub OverviewHeadingCompact<FieldInspectorState> {} |_c, _state, _cx| {} => OverviewHeadingSummary() }
