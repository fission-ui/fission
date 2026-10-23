use crate::catalog::TargetFilter;
use crate::state::{on_filter_changed, FilterChanged, ShowcaseState};
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;
use fission::widgets::Tag;

fission_component! {
/// Narrows the catalog to examples for one kind of platform.
///
/// Each filter is a selectable tag, which reports whether it is the active one.
    #[derive(Clone, Debug)] pub(crate) FilterBar<ShowcaseState> {}
    |_component, state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Row(
            children: [
                for filter in TargetFilter::ALL.iter().copied() {
                    Tag(
                        label: cx.view.tr(filter.translation_key()),
                        on_press: with_reducer!(cx.build, FilterChanged(filter), on_filter_changed),
                        selected: filter == state.target_filter,
                        key: filter.translation_key(),
                    )
                },
            ],
            gap: tokens.spacing.xs,
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
        )
}
