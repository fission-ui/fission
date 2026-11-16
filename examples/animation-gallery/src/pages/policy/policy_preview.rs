use crate::state::{AnimationGalleryState, MotionPolicy};
use crate::style;
use crate::widgets::common::{preview_active, PreviewShell};
use fission::prelude::*;
use fission::widgets::{Toast, ToastKind, ToastMotion};

fission_component! {
    pub(super) PolicyPreview<AnimationGalleryState> |_preview, state, cx| {} =>
        PreviewShell(
            if preview_active(state) {
            Toast(
                id: WidgetId::explicit("gallery.policy.toast"),
                kind: ToastKind::Success,
                message: "Policy is evaluating the same ToastMotion source.".to_string(),
                duration: fission::widgets::ToastDuration::Default,
                motion: policy_toast_motion(state.policy),
            )
        } else {
            Text(
                "Use the playback control to run the real Toast widget under the selected policy.",
            )
            .size(cx.view.env().theme.tokens.typography.font_size_xs)
            .color(style::text_muted())
        })
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
