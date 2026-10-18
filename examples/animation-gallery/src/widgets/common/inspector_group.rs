use crate::state::AnimationGalleryState;
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub(super) InspectorGroup<AnimationGalleryState> { title: String, rows: Vec<String> }
    |group, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(gap: tokens.spacing.xs, children: [
            Text(group.title.clone()).size(tokens.typography.font_size_sm).color(style::text_primary()),
            for (index, label) in group.rows.iter().enumerate() {
                LabelRow(label.clone(), key: index)
            }
        ])
}
