use crate::app::StoreState;
use fission::prelude::*;
fission_component! { pub FeaturePill<StoreState> { label: &'static str, accent: Color } |pill, _state, cx| { let t=&cx.view.env().theme.tokens; } =>
    Container(Text(pill.label).size(t.typography.font_size_xs).line_height(t.typography.font_size_xs*t.typography.line_height_snug).weight(t.typography.font_weight_semibold).color(t.colors.text_secondary))
        .padding([t.spacing.s,t.spacing.s,t.spacing.xs,t.spacing.xs]).border(pill.accent.with_alpha(160),1.0).border_radius(t.radii.full).bg(pill.accent.with_alpha(34))
}
