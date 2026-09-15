use super::host_only_preview::HostOnlyPreview;
use fission::icons::material;
use fission::prelude::*;

/// A native web view is a host window, which the showcase cannot mount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EmbedWebViewExample;

impl From<EmbedWebViewExample> for Widget {
    fn from(_component: EmbedWebViewExample) -> Self {
        HostOnlyPreview {
            slug: "embed-webview",
            description_key: "showcase.host_preview.embed_webview",
            icon: || Icon::svg(material::action::language::round()),
        }
        .into()
    }
}
