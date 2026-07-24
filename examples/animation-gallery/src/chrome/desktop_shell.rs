use super::*;

pub(super) struct DesktopShell<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<DesktopShell<'_>> for Widget {
    fn from(shell: DesktopShell<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Row {
            gap: Some(tokens.spacing.none),
            align_items: fission::op::AlignItems::Start,
            children: vec![
                BrandRail {
                    ctx: shell.ctx,
                    state: shell.state,
                }
                .into(),
                Container::new(Scroll {
                    id: Some(WidgetId::explicit(&format!(
                        "animation-gallery.content-scroll.{}",
                        shell.state.current_path
                    ))),
                    direction: FlexDirection::Column,
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    child: Some(
                        Container::new(Column {
                            gap: Some(tokens.spacing.m),
                            children: vec![
                                HeroBar { state: shell.state }.into(),
                                pages::GalleryRouter {
                                    current_path: shell.state.current_path.clone(),
                                }
                                .into(),
                                BottomStrip.into(),
                            ],
                            ..Default::default()
                        })
                        .width_length(Length::percent(100.0))
                        .padding_all(tokens.spacing.l)
                        .into(),
                    ),
                    ..Default::default()
                })
                .flex_grow(1.0)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
