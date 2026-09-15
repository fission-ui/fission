use crate::catalog::TargetFilter;
use crate::state::{on_filter_changed, FilterChanged, ShowcaseState};
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;
use fission::widgets::Tag;

/// Narrows the catalog to examples for one kind of platform.
///
/// Each filter is a selectable tag, which reports whether it is the active one.
#[derive(Clone, Debug)]
pub(crate) struct FilterBar;

impl From<FilterBar> for Widget {
    fn from(_component: FilterBar) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let children = TargetFilter::ALL
            .iter()
            .map(|filter| {
                Tag {
                    label: view.tr(filter.translation_key()),
                    on_close: None,
                    on_press: Some(with_reducer!(
                        ctx,
                        FilterChanged(*filter),
                        on_filter_changed
                    )),
                    selected: *filter == view.state().target_filter,
                }
                .into()
            })
            .collect();

        Row {
            children,
            gap: Some(tokens.spacing.xs),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into()
    }
}
