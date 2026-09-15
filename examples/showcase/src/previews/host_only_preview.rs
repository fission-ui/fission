use crate::catalog::example_by_slug;
use crate::state::ShowcaseState;
use fission::prelude::*;
use fission::widgets::{Center, Code, EmptyState};

/// Stands in for an example whose surface only its own host can draw.
///
/// Video playback, native web views and terminal sessions need the platform
/// window or process they run in, so a mounted copy inside the showcase would be
/// a blank rectangle. Instead the preview says what the surface shows, why it is
/// not here, and how to run it.
#[derive(Clone, Copy, Debug)]
pub(super) struct HostOnlyPreview {
    /// The catalog slug of the example, used to find its run command.
    pub(super) slug: &'static str,
    /// The translation key describing what the surface would show.
    pub(super) description_key: &'static str,
    /// The icon for the kind of surface.
    pub(super) icon: fn() -> Icon,
}

impl From<HostOnlyPreview> for Widget {
    fn from(component: HostOnlyPreview) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let example = example_by_slug(component.slug);
        let description = format!(
            "{} {}",
            view.tr(component.description_key),
            view.tr("showcase.host_preview.reason")
        );

        Container::new(Center {
            child: EmptyState {
                icon: Some((component.icon)().into()),
                title: view.tr("showcase.host_preview.title"),
                description: Some(description),
                action: Some(
                    SemanticsRegion::new(Code {
                        text: example.command.to_string(),
                    })
                    .label(view.tr("showcase.host_preview.command"))
                    .identifier(format!("showcase.host_preview.{}.command", component.slug))
                    .into(),
                ),
            }
            .into(),
        })
        .width_length(Length::percent(100.0))
        .height_length(Length::percent(100.0))
        .padding_all(tokens.spacing.l)
        .into()
    }
}
