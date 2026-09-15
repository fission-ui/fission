use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::prelude::*;
use fission::widgets::{Center, EmptyState};

/// Shown when an address matches no example, with a way back to the catalog.
#[derive(Clone, Debug)]
pub(crate) struct NotFoundPage;

impl From<NotFoundPage> for Widget {
    fn from(_component: NotFoundPage) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        Center {
            child: EmptyState {
                icon: None,
                title: view.tr("showcase.not_found.title"),
                description: Some(view.tr("showcase.not_found.description")),
                action: Some(
                    Button {
                        variant: ButtonVariant::Outline,
                        child: Some(
                            Text::new(TextContent::Key("showcase.not_found.back".into())).into(),
                        ),
                        on_press: Some(with_reducer!(ctx, Navigate("/".into()), on_navigate)),
                        ..Default::default()
                    }
                    .semantics_identifier("showcase.not_found.back")
                    .into(),
                ),
            }
            .into(),
        }
        .into()
    }
}
