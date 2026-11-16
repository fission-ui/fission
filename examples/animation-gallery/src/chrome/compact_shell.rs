use super::*;

fission_component! {
    pub(super) CompactShell<AnimationGalleryState> |_shell, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Scroll(
                id: WidgetId::explicit(&format!(
                    "animation-gallery.compact-scroll.{}",
                    state.current_path,
                )),
                direction: FlexDirection::Column,
                show_scrollbar: true,
                flex_grow: 1.0_f32,
                child: Container(
                    Column(
                        gap: tokens.spacing.m,
                        children: [
                            if !state.embedded { CompactBrand() },
                            if state.current_path == routes::OVERVIEW { TopStoryRow() },
                            Dashboard(),
                            BottomStrip(),
                        ],
                    ),
                )
                .width_length(Length::percent(100.0))
                .padding_all(tokens.spacing.m),
            ),
        ).flex_grow(1.0)
}
