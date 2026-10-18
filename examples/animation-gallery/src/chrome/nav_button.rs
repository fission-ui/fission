use super::*;

fission_component! {
    pub(super) NavButton<AnimationGalleryState> { item: &'static NavItem }
    |button, state, cx| {} =>
        Button(
            variant: if state.current_path == button.item.path {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Ghost
            },
            child: Text(button.item.label)
                .size(cx.view.env().theme.tokens.typography.font_size_xs),
            on_press: cx.bind(
                NavigateTo(button.item.path.to_string()),
                fission::reduce_with!(navigate_to),
            ),
        )
        .semantics_identifier(format!(
            "animation-gallery.nav{}",
            button.item.path.replace('/', "."),
        ))
        .width_length(Length::percent(100.0))
}
