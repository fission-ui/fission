use super::*;

fission_component! {
    pub(super) DesktopShell<AnimationGalleryState> |_shell, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.none,
            align_items: fission::op::AlignItems::Start,
            children: [
                BrandRail(),
                Container(
                    Scroll(
                        id: WidgetId::explicit(&format!(
                            "animation-gallery.content-scroll.{}",
                            state.current_path,
                        )),
                        direction: FlexDirection::Column,
                        show_scrollbar: true,
                        flex_grow: 1.0_f32,
                        child: Container(
                            Column(
                                gap: tokens.spacing.m,
                                children: [
                                    if !state.embedded { HeroBar() },
                                    pages::GalleryRouter::new(state.current_path.clone()),
                                    BottomStrip(),
                                ],
                            ),
                        )
                        .width_length(Length::percent(100.0))
                        .padding_all(tokens.spacing.l),
                    ),
                ).flex_grow(1.0),
            ],
        )
}
