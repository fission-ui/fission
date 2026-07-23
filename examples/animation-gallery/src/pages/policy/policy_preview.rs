use crate::state::{AnimationGalleryState, MotionPolicy};
use crate::style::MUTED;
use crate::widgets::common::{preview_active, PreviewShell};
use fission::prelude::*;
use fission::widgets::{Toast, ToastKind, ToastMotion};

pub(super) struct PolicyPreview<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<PolicyPreview<'_>> for Widget {
    fn from(preview: PolicyPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let child: Widget = if preview_active(preview.state) {
            Toast {
                id: WidgetId::explicit("gallery.policy.toast"),
                kind: ToastKind::Success,
                message: "Policy is evaluating the same ToastMotion source.".into(),
                on_close: None,
                motion: policy_toast_motion(preview.state.policy),
            }
            .into()
        } else {
            Text::new(
                "Use the playback control to run the real Toast widget under the selected policy.",
            )
            .size(view.env().theme.tokens.typography.font_size_xs)
            .color(MUTED)
            .into()
        };

        PreviewShell { child }.into()
    }
}

fn policy_toast_motion(policy: MotionPolicy) -> Option<ToastMotion> {
    match policy {
        MotionPolicy::Full => {
            Some(ToastMotion::SlideFromTop + ToastMotion::Fade + ToastMotion::Pop)
        }
        MotionPolicy::Reduced => Some(ToastMotion::Fade),
        MotionPolicy::Disabled => None,
    }
}
