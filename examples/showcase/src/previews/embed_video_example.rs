use super::host_only_preview::HostOnlyPreview;
use fission::icons::material;
use fission::prelude::*;

/// Video playback is drawn by the host's media layer, which the showcase cannot mount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EmbedVideoExample;

impl From<EmbedVideoExample> for Widget {
    fn from(_component: EmbedVideoExample) -> Self {
        HostOnlyPreview {
            slug: "embed-video",
            description_key: "showcase.host_preview.embed_video",
            icon: || Icon::svg(material::av::videocam::round()),
        }
        .into()
    }
}
