use fission::prelude::*;

use crate::GalleryState;

fission_component! {
    pub(super) ProfileFooter<GalleryState> { action: ActionEnvelope }
    |footer, _state, _cx| {} =>
        Row(children: [
            Button(
                variant: ButtonVariant::TertiaryGray,
                size: ComponentSize::Md,
                content: ButtonContent::new("Cancel"),
                on_press: footer.action.clone(),
            )
            .semantics_identifier("quality-gallery.profile.cancel"),
            Spacer(flex_grow: 1.0_f32),
            Button(
                variant: ButtonVariant::Primary,
                size: ComponentSize::Md,
                content: ButtonContent::new("Save changes")
                    .leading_icon(Icon::svg(material::content::save::regular())),
                on_press: footer.action.clone(),
            )
            .semantics_identifier("quality-gallery.profile.save"),
        ])
}
