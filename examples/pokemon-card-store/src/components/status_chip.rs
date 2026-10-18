use crate::app::StoreState;
use fission::prelude::*;
fission_component! { pub StatusChip<StoreState> { label: &'static str, identifier: &'static str, status: &'static str } |chip, _state, cx| { let t=&cx.view.env().theme.tokens; } =>
    Column(gap:t.spacing.xs, children:[
        Text(chip.label).size(t.typography.font_size_xs).line_height(t.typography.font_size_xs*t.typography.line_height_snug).weight(t.typography.font_weight_bold).color(t.colors.text_muted),
        Text(chip.status).size(t.typography.body_medium_size).line_height(t.typography.body_medium_size*t.typography.line_height_snug).weight(t.typography.font_weight_bold).color(t.colors.text_secondary).semantics_identifier(chip.identifier),
    ])
}
