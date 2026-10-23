use super::*;

fission_component! {
    pub(super) DashboardCompact<AnimationGalleryState> |_dashboard, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.m,
            children: [
                CompactNavPanel(),
                Container(pages::GalleryRouter::new(state.current_path.clone()))
                    .width_length(Length::percent(100.0)),
            ],
        )
}
