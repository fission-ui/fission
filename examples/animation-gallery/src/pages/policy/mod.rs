mod policy_card;
mod policy_cards;
mod policy_preview;

use crate::state::{AnimationGalleryState, MotionChoice};
use crate::ui;
use crate::widgets::common::ControlsPanel;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use policy_cards::PolicyCards;
use policy_preview::PolicyPreview;

pub struct PolicyPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PolicyPage<'_>> for Widget {
    fn from(page: PolicyPage<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let spacing = &view.env().theme.tokens.spacing;

        Column {
            gap: Some(spacing.s),
            children: widgets![
                ui::PageHeader {
                    title: "Motion Policy",
                    subtitle: "Preview how one source-level motion declaration evaluates under full, reduced, or disabled policy.",
                },
                ControlsPanel {
                    ctx: &page.ctx,
                    state: page.state,
                    motions: &[MotionChoice::Composition],
                },
                PolicyPreview { state: page.state },
                PolicyCards {
                    selected: page.state.policy,
                },
                ui::CodeBlock {
                    source: POLICY_SOURCE,
                },
                ui::PageNote {
                    title: "Accessibility first",
                    body: "Policy changes evaluation, not source structure. The tree can still contain motion: Some(...), while the runtime shortens, reduces, or snaps interpolation.",
                },
            ],
            ..Default::default()
        }
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
