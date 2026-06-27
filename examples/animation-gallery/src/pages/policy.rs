use crate::state::{AnimationGalleryState, MotionPolicy};
use crate::style::*;
use crate::ui;
use crate::widgets::common::{preview_active, ControlsPanel, PreviewShell};
use fission::build::BuildCtxHandle;
use fission::widgets::{Toast, ToastKind, ToastMotion};
use fission::{Column, Container, Row, Text, Widget, WidgetId};

pub struct PolicyPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PolicyPage<'_>> for Widget {
    fn from(page: PolicyPage<'_>) -> Self {
        Column {
            gap: Some(14.0),
            children: vec![
                ui::PageHeader {
                    title: "Motion Policy",
                    subtitle: "Preview how one source-level motion declaration evaluates under full, reduced, or disabled policy.",
                }
                .into(),
                ControlsPanel {
                    ctx: &page.ctx,
                    state: page.state,
                    motions: &[crate::state::MotionChoice::Composition],
                }
                .into(),
                PolicyPreview { state: page.state }.into(),
                Container::new(Row {
                    gap: Some(14.0),
                    children: vec![
                        PolicyCard {
                            title: "Full",
                            body: "FromTop + Fade + Scale",
                            active: page.state.policy == MotionPolicy::Full,
                        }
                        .into(),
                        PolicyCard {
                            title: "Reduced",
                            body: "Fade only, shorter duration",
                            active: page.state.policy == MotionPolicy::Reduced,
                        }
                        .into(),
                        PolicyCard {
                            title: "Disabled",
                            body: "Instant final state",
                            active: page.state.policy == MotionPolicy::Disabled,
                        }
                        .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(16.0)
                .border(BORDER, 1.0)
                .border_radius(16.0)
                .bg(SURFACE)
                .into(),
                ui::CodeBlock {
                    source: POLICY_SOURCE,
                }
                .into(),
                ui::PageNote {
                    title: "Accessibility first",
                    body: "Policy changes evaluation, not source structure. The tree can still contain motion: Some(...), while the runtime shortens, reduces, or snaps interpolation.",
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct PolicyPreview<'a> {
    state: &'a AnimationGalleryState,
}

impl From<PolicyPreview<'_>> for Widget {
    fn from(preview: PolicyPreview<'_>) -> Self {
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
            .size(12.0)
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

struct PolicyCard<'a> {
    title: &'a str,
    body: &'a str,
    active: bool,
}

impl From<PolicyCard<'_>> for Widget {
    fn from(card: PolicyCard<'_>) -> Self {
        Container::new(Column {
            gap: Some(8.0),
            children: vec![
                Container::new(Text::new(" "))
                    .height(44.0)
                    .border_radius(10.0)
                    .bg(if card.active {
                        SOFT_BLUE
                    } else {
                        color(244, 246, 250, 255)
                    })
                    .into(),
                Text::new(card.title).size(13.0).color(INK).into(),
                Text::new(card.body).size(11.0).color(MUTED).into(),
            ],
            ..Default::default()
        })
        .width(180.0)
        .padding_all(12.0)
        .border(if card.active { BLUE } else { BORDER }, 1.0)
        .border_radius(14.0)
        .bg(SURFACE)
        .into()
    }
}

const POLICY_SOURCE: &str = r#"pub enum MotionPolicy {
    Full,
    Reduced,
    Disabled,
}

motion: Some(ToastMotion::SlideFromTop + ToastMotion::Fade + ToastMotion::Pop)

// Source intent remains the same. The gallery maps policy to full, reduced, or no motion."#;
