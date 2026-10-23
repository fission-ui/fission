use crate::state::{policy_label, AnimationGalleryState};
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub CurrentValues<AnimationGalleryState> {}
    |_values, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let progress = (state.scrub_ms as f32 / 300.0).clamp(0.0, 1.0);
        let progress_value = format!("{progress:.2}");
        let opacity_value = format!("{:.2}", 0.35 + 0.65 * progress);
        let scale_value = format!("{:.2}", 0.96 + 0.04 * progress);
        let translation_value = format!("{}px", (-24.0 + 24.0 * progress).round() as i32);
    } =>
        Column(
            gap: tokens.spacing.xs,
            children: [
                Text("Current Values").size(tokens.typography.font_size_sm).color(style::text_primary()),
                LabelValue("t(progress)".to_string(), progress_value),
                LabelValue("opacity".to_string(), opacity_value),
                LabelValue("scale".to_string(), scale_value),
                LabelValue("translateY".to_string(), translation_value),
                LabelValue("policy".to_string(), policy_label(state.policy).to_string()),
            ],
        )
}
