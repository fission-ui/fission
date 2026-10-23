use super::*;

fission_component! {
    pub(super) NavChip<AnimationGalleryState> {
        label: &'static str,
        path: &'static str,
    } |chip, state, cx| {} =>
        Button(
            width: NAV_CHIP_WIDTH,
            variant: if state.current_path == chip.path {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Outline
            },
            child: Text(chip.label)
                .size(cx.view.env().theme.tokens.typography.font_size_xs),
            on_press: cx.bind(
                NavigateTo(chip.path.to_string()),
                fission::reduce_with!(navigate_to),
            ),
        ).semantics_identifier(format!(
            "animation-gallery.nav{}",
            chip.path.replace('/', "."),
        ))
}
