use crate::i18n::message;
use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::op::Fill;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct NavItem {
    pub(super) label_key: &'static str,
    pub(super) path: &'static str,
}

impl From<NavItem> for Widget {
    fn from(component: NavItem) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let selected = view.state().current_path == component.path
            || (component.path == "/" && view.state().current_path.starts_with("/examples/"));
        let navigate = with_reducer!(ctx, Navigate(component.path.into()), on_navigate);
        let label = message(view.env(), component.label_key);
        let identifier = format!(
            "showcase.nav.{}",
            component.label_key.trim_start_matches("showcase.nav.")
        );

        Pressable::new(
            Text::new(TextContent::Key(component.label_key.into()))
                .size(tokens.typography.font_size_sm)
                .weight(if selected {
                    tokens.typography.font_weight_bold
                } else {
                    tokens.typography.font_weight_medium
                })
                .color(if selected {
                    tokens.colors.primary
                } else {
                    tokens.colors.text_secondary
                }),
        )
        .id(WidgetId::explicit(&identifier))
        .on_press(navigate)
        .role(PressableRole::Link)
        .label(label)
        .semantics_identifier(identifier)
        .style(PressableStyle {
            padding: Some([
                Length::points(tokens.spacing.s),
                Length::points(tokens.spacing.s),
                Length::points(tokens.spacing.m),
                Length::points(tokens.spacing.m),
            ]),
            corner_radius: Some(tokens.radii.medium),
            background: selected.then(|| Fill::Solid(tokens.colors.primary_subtle)),
            ..Default::default()
        })
        .hover(PressableStyle {
            background: Some(Fill::Solid(tokens.colors.surface_sunken)),
            ..Default::default()
        })
        .into()
    }
}
