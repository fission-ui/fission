use super::host_only_preview::HostOnlyPreview;
use fission::icons::material;
use fission::prelude::*;

/// A terminal session needs its own shell process and host, which the showcase cannot mount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalExample;

impl From<TerminalExample> for Widget {
    fn from(_component: TerminalExample) -> Self {
        HostOnlyPreview {
            slug: "terminal",
            description_key: "showcase.host_preview.terminal",
            icon: || Icon::svg(material::action::terminal::round()),
        }
        .into()
    }
}
