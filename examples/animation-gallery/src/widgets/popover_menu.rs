use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const MENU_WIDTH: f32 = 260.0;

fission_component! {
    pub(super) PopoverMenu<AnimationGalleryState> { close: ActionEnvelope }
    |menu, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
            gap: tokens.spacing.xs,
            children: [
                Text("Invite teammate").size(tokens.typography.font_size_sm),
                Text("Manage permissions").size(tokens.typography.font_size_sm),
                Text("Archive workspace").size(tokens.typography.font_size_sm),
                Button(
                    variant: ButtonVariant::Outline,
                    child: Text("Close popover"),
                    on_press: menu.close.clone(),
                )
                .semantics_identifier("gallery.real.popover.close"),
            ],
        ))
        .padding_all(tokens.spacing.s)
        .width(MENU_WIDTH)
        .border_radius(tokens.radii.large)
        .bg(style::surface())
}
