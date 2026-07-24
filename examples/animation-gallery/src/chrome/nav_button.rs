use super::*;

pub(super) struct NavButton<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
    pub(super) item: &'a NavItem,
}

impl From<NavButton<'_>> for Widget {
    fn from(button: NavButton<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let active = button.state.current_path == button.item.path;
        Button {
            variant: if active {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Ghost
            },
            child: Some(
                Text::new(button.item.label)
                    .size(view.env().theme.tokens.typography.font_size_xs)
                    .into(),
            ),
            on_press: Some(button.ctx.bind(
                NavigateTo(button.item.path.to_string()),
                fission::reduce_with!(navigate_to),
            )),
            ..Default::default()
        }
        .into()
    }
}
