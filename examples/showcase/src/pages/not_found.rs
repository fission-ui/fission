use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::prelude::*;
use fission::widgets::{Center, EmptyState};

fission_component! {
/// Shown when an address matches no example, with a way back to the catalog.
    #[derive(Clone, Debug)] pub(crate) NotFoundPage<ShowcaseState> {}
    |_component, _state, cx| {} =>
        Center(
            EmptyState(
                title: cx.view.tr("showcase.not_found.title"),
                description: cx.view.tr("showcase.not_found.description"),
                action: Button(
                        variant: ButtonVariant::Outline,
                        child: Text(TextContent::Key("showcase.not_found.back".into())),
                        on_press: with_reducer!(cx.build, Navigate("/".into()), on_navigate),
                    ).semantics_identifier("showcase.not_found.back"),
            ),
        )
}
