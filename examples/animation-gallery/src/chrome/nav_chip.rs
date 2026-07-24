use super::*;

pub(super) struct NavChip<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
    pub(super) label: &'a str,
    pub(super) path: &'a str,
}

impl From<NavChip<'_>> for Widget {
    fn from(chip: NavChip<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let active = chip.state.current_path == chip.path;
        Button {
            width: Some(NAV_CHIP_WIDTH),
            variant: if active {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Outline
            },
            child: Some(
                Text::new(chip.label)
                    .size(view.env().theme.tokens.typography.font_size_xs)
                    .into(),
            ),
            on_press: Some(chip.ctx.bind(
                NavigateTo(chip.path.to_string()),
                fission::reduce_with!(navigate_to),
            )),
            ..Default::default()
        }
        .into()
    }
}
