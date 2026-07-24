use crate::components::overview_heading_summary::OverviewHeadingSummary;
use fission::prelude::*;

pub struct OverviewHeadingCompact;

impl From<OverviewHeadingCompact> for Widget {
    fn from(_: OverviewHeadingCompact) -> Self {
        OverviewHeadingSummary.into()
    }
}
