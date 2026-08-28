use fission_core::ui::{Button, ButtonVariant, Text, Widget};
use fission_core::ActionEnvelope;
use fission_core::{Action, Hyperlink, NavigationCommand, NavigationRequested};
use serde::{Deserialize, Serialize};

/// Text link backed by either a normal action or Fission navigation metadata.
///
/// Use [`Link::to`] or [`Link::hyperlink`] for navigation so Web, Static site,
/// and SSR can lower a real `href`; use `on_click` directly for a non-navigation
/// action that merely uses link styling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    /// Visible link label.
    pub text: String,
    /// Action dispatched when activated; navigation actions also carry hyperlink metadata.
    pub on_click: Option<ActionEnvelope>,
}

impl Link {
    /// Creates a genuine navigational link while preserving the existing
    /// action-backed representation of `Link`.
    pub fn to(text: impl Into<String>, href: impl Into<String>) -> Self {
        Self::hyperlink(text, Hyperlink::new(href))
    }

    /// Creates a link with complete target/relationship/download metadata.
    pub fn hyperlink(text: impl Into<String>, hyperlink: Hyperlink) -> Self {
        Self {
            text: text.into(),
            on_click: Some(NavigationRequested::new(NavigationCommand::Open(hyperlink)).into()),
        }
    }
}

impl From<Link> for Widget {
    fn from(component: Link) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let child = Text::new(this.text.clone())
            .color(tokens.colors.primary)
            .underline(true);
        let hyperlink = this.on_click.as_ref().and_then(|action| {
            (action.id == NavigationRequested::static_id())
                .then(|| serde_json::from_slice::<NavigationRequested>(&action.payload).ok())
                .flatten()
                .and_then(|request| match request.command {
                    NavigationCommand::Open(hyperlink) => Some(hyperlink),
                    NavigationCommand::Push(path) | NavigationCommand::Replace(path) => {
                        Some(Hyperlink::new(path))
                    }
                    _ => None,
                })
        });

        if let Some(hyperlink) = hyperlink {
            fission_core::ui::Pressable::new(child)
                .hyperlink(hyperlink)
                .on_press(this.on_click.clone().expect("navigation action is present"))
                .layout(fission_ir::op::BoxStyle::default())
                .into()
        } else {
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(child.into()),
                on_press: this.on_click.clone(),
                content_align: fission_core::ui::ButtonContentAlign::Start,
                padding: Some([0.0; 4]), // Minimal padding
                ..Default::default()
            }
            .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::internal::{BuildCtx, InternalLoweringCx};
    use fission_core::{build, Env, LinkTarget, RuntimeState, View};
    use fission_ir::Op;

    #[test]
    fn navigational_link_lowers_complete_hyperlink_semantics() {
        let env = Env::default();
        let runtime = RuntimeState::default();
        let state = ();
        let view = View::new(&state, &runtime, &env, None);
        let mut build_ctx = BuildCtx::<()>::new();
        let widget = build::enter(&mut build_ctx, &view, || {
            Link::hyperlink(
                "Open report",
                Hyperlink::new("/reports/42")
                    .target(LinkTarget::NewWindow)
                    .rel("alternate")
                    .download("report.pdf"),
            )
            .into()
        });
        let mut lowering = InternalLoweringCx::new(&env, &runtime, None, None);
        fission_core::internal::lower_widget(&widget, &mut lowering);

        let link = lowering
            .ir
            .nodes
            .values()
            .find_map(|node| match &node.op {
                Op::Semantics(semantics) => semantics.hyperlink.as_ref(),
                _ => None,
            })
            .expect("Link should lower genuine hyperlink metadata");
        assert_eq!(link.href, "/reports/42");
        assert_eq!(link.target, LinkTarget::NewWindow);
        assert_eq!(link.rel.as_deref(), Some("alternate"));
        assert_eq!(link.download.as_deref(), Some("report.pdf"));
    }
}
