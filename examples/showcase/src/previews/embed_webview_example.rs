use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use embed_webview_example::{WebViewEmbedApp, WebViewEmbedState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EmbedWebViewExample;

impl From<EmbedWebViewExample> for Widget {
    fn from(_component: EmbedWebViewExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<WebViewEmbedState, _>::new(
            "showcase.example.embed-webview",
            view.state().preview_generation,
            WebViewEmbedApp,
        )
        .into()
    }
}
