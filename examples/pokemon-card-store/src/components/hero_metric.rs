use crate::app::StoreState;
use fission::prelude::*;
fission_component! { pub HeroMetric<StoreState> { value: &'static str, label: &'static str } |metric, _state, cx| { let t=&cx.view.env().theme.tokens; } =>
    Container(Column(gap:t.spacing.xs, children:[
        Text(metric.value).size(t.typography.font_size_xl).line_height(t.typography.font_size_xl*t.typography.line_height_heading).weight(t.typography.font_weight_bold).color(t.colors.text_primary),
        Text(metric.label).size(t.typography.font_size_xs).line_height(t.typography.font_size_xs*t.typography.line_height_snug).color(t.colors.text_muted),
    ])).padding_all(t.spacing.s).border(t.colors.border,1.0).border_radius(t.radii.xl).bg(t.colors.surface)
}
