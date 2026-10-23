mod policy_card;
mod policy_cards;
mod policy_preview;

use crate::state::{AnimationGalleryState, MotionChoice};
use crate::ui::*;
use crate::widgets::common::ControlsPanel;
use fission::prelude::*;
use policy_cards::PolicyCards;
use policy_preview::PolicyPreview;

fission_component! {
    pub PolicyPage<AnimationGalleryState> |_page, state, cx| {
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        Column(
            gap: spacing.s,
            children: [
                PageHeader(
                    "Motion Policy".to_string(),
                    "Preview how one source-level motion declaration evaluates under full, reduced, or disabled policy.".to_string(),
                ),
                ControlsPanel(vec![MotionChoice::Composition]),
                PolicyPreview(),
                PolicyCards(state.policy),
                CodeBlock(POLICY_SOURCE.to_string()),
                PageNote(
                    "Accessibility first".to_string(),
                    "Policy changes evaluation, not source structure. The tree can still contain motion: Some(...), while the runtime shortens, reduces, or snaps interpolation.".to_string(),
                ),
            ],
        )
}

const POLICY_SOURCE: &str = r#"pub enum MotionPolicy {
    Full,
    Reduced,
    Disabled,
}

motion: Some(ToastMotion::SlideFromTop + ToastMotion::Fade + ToastMotion::Pop)

// Source intent remains the same. The gallery maps policy to full, reduced, or no motion."#;
