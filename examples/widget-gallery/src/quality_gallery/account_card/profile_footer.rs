use fission::prelude::*;

pub(super) struct ProfileFooter {
    pub action: ActionEnvelope,
}

impl From<ProfileFooter> for CardFooter {
    fn from(footer: ProfileFooter) -> Self {
        CardFooter::new(widgets![
            Button {
                variant: ButtonVariant::TertiaryGray,
                size: ComponentSize::Md,
                content: Some(ButtonContent::new("Cancel")),
                on_press: Some(footer.action.clone()),
                ..Default::default()
            }
            .semantics_identifier("quality-gallery.profile.cancel"),
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            },
            Button {
                variant: ButtonVariant::Primary,
                size: ComponentSize::Md,
                content: Some(
                    ButtonContent::new("Save changes")
                        .leading_icon(Icon::svg(material::content::save::regular())),
                ),
                on_press: Some(footer.action),
                ..Default::default()
            }
            .semantics_identifier("quality-gallery.profile.save"),
        ])
    }
}
