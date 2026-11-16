use crate::layout::BREADCRUMB_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) Breadcrumb<EditorState> |_breadcrumb, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
    if state.open_tabs.is_empty() || state.breadcrumb_path.is_empty() {
        Spacer(height: 0.0)
    } else {
        Container(
            Row(
                align_items: ir_op::AlignItems::Center,
                children: [
                    for (index, segment) in state.breadcrumb_path.iter().enumerate() {
                        BreadcrumbSegment(index, segment.clone(), key: index)
                    },
                ],
            ),
        )
        .height(BREADCRUMB_HEIGHT)
        .padding_all(tokens.spacing.xs)
        .bg(palette.surface_bg)
        .flex_shrink(0.0)
    }
}

fission_component! {
    BreadcrumbSegment<EditorState> {
        index: usize,
        segment: String,
    } |segment, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Row(
        children: [
            if segment.index > 0 {
                Text(" > ")
                    .size(tokens.typography.font_size_xs)
                    .color(palette.dim_text)
            },
            Text(segment.segment.clone())
                .size(tokens.typography.font_size_xs)
                .color(palette.dim_text),
        ],
    )
}
