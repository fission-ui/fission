use crate::state::AnimationGalleryState;
use crate::style::SURFACE;
use fission::prelude::*;

const MENU_WIDTH: f32 = 260.0;

pub(super) struct PopoverMenu {
    pub close: ActionEnvelope,
}

impl From<PopoverMenu> for Widget {
    fn from(menu: PopoverMenu) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new("Invite teammate").size(tokens.typography.font_size_sm),
                Text::new("Manage permissions").size(tokens.typography.font_size_sm),
                Text::new("Archive workspace").size(tokens.typography.font_size_sm),
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Text::new("Close popover").into()),
                    on_press: Some(menu.close),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .width(MENU_WIDTH)
        .border_radius(tokens.radii.large)
        .bg(SURFACE)
        .into()
    }
}
