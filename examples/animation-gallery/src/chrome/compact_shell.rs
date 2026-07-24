use super::*;

pub(super) struct CompactShell<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<CompactShell<'_>> for Widget {
    fn from(shell: CompactShell<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut children: Vec<Widget> = vec![CompactBrand.into()];
        if shell.state.current_path == routes::OVERVIEW {
            children.push(
                TopStoryRow {
                    ctx: shell.ctx,
                    state: shell.state,
                }
                .into(),
            );
        }
        children.push(
            Dashboard {
                ctx: shell.ctx,
                state: shell.state,
            }
            .into(),
        );
        children.push(BottomStrip.into());

        Container::new(Scroll {
            id: Some(WidgetId::explicit(&format!(
                "animation-gallery.compact-scroll.{}",
                shell.state.current_path
            ))),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            child: Some(
                Container::new(Column {
                    gap: Some(tokens.spacing.m),
                    children,
                    ..Default::default()
                })
                .width_length(Length::percent(100.0))
                .padding_all(tokens.spacing.m)
                .into(),
            ),
            ..Default::default()
        })
        .flex_grow(1.0)
        .into()
    }
}
