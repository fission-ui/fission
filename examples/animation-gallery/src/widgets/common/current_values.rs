use crate::state::{policy_label, AnimationGalleryState};
use crate::style::INK;
use crate::ui;
use fission::prelude::*;

pub struct CurrentValues<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<CurrentValues<'_>> for Widget {
    fn from(values: CurrentValues<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let progress = (values.state.scrub_ms as f32 / 300.0).clamp(0.0, 1.0);
        let progress_value = format!("{progress:.2}");
        let opacity_value = format!("{:.2}", 0.35 + 0.65 * progress);
        let scale_value = format!("{:.2}", 0.96 + 0.04 * progress);
        let translation_value = format!("{}px", (-24.0 + 24.0 * progress).round() as i32);

        Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new("Current Values")
                    .size(tokens.typography.font_size_sm)
                    .color(INK),
                ui::LabelValue {
                    label: "t(progress)",
                    value: &progress_value,
                },
                ui::LabelValue {
                    label: "opacity",
                    value: &opacity_value,
                },
                ui::LabelValue {
                    label: "scale",
                    value: &scale_value,
                },
                ui::LabelValue {
                    label: "translateY",
                    value: &translation_value,
                },
                ui::LabelValue {
                    label: "policy",
                    value: policy_label(values.state.policy),
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
