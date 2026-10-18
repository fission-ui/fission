use super::*;

fission_component! {
    pub(super) DashboardExpanded<AnimationGalleryState> |_dashboard, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.m,
            align_items: fission::op::AlignItems::Start,
            children: [
                NavPanel(),
                Container(pages::GalleryRouter::new(state.current_path.clone()))
                    .min_width(MIN_ROUTED_CONTENT_WIDTH)
                    .flex_grow(1.0),
            ],
        )
}
