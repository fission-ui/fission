use crate::catalog::TargetFilter;
use crate::i18n::message;
use crate::state::{on_filter_changed, FilterChanged, ShowcaseState};
use fission::op::{AlignItems, Fill, FlexWrap};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct FilterBar;

impl From<FilterBar> for Widget {
    fn from(_component: FilterBar) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let children = TargetFilter::ALL
            .iter()
            .map(|filter| {
                let selected = *filter == view.state().target_filter;
                let select = with_reducer!(ctx, FilterChanged(*filter), on_filter_changed);
                let key = filter.translation_key();
                Pressable::new(
                    Text::new(TextContent::Key(key.into()))
                        .size(tokens.typography.font_size_sm)
                        .weight(if selected {
                            tokens.typography.font_weight_bold
                        } else {
                            tokens.typography.font_weight_medium
                        })
                        .color(if selected {
                            tokens.colors.primary
                        } else {
                            tokens.colors.text_muted
                        }),
                )
                .id(WidgetId::explicit(&format!("showcase.filter.{:?}", filter)))
                .on_press(select)
                .label(message(view.env(), key))
                .semantics_identifier(format!(
                    "showcase.filter.{}",
                    key.trim_start_matches("showcase.catalog.filter.")
                ))
                .style(PressableStyle {
                    padding: Some(Length::all(Length::points(tokens.spacing.s))),
                    corner_radius: Some(tokens.radii.full),
                    background: selected.then(|| Fill::Solid(tokens.colors.primary_subtle)),
                    ..Default::default()
                })
                .hover(PressableStyle {
                    background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                    ..Default::default()
                })
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
