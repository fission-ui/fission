use fission::prelude::*;

use crate::GalleryState;

#[fission_reducer(AddQualityDevice)]
fn add_quality_device(_state: &mut GalleryState) {}

fission_component! {
    TrustedDevicesEmpty<GalleryState> { compact: bool }
    |empty, _state, cx| {} =>
        Container(EmptyState(
            icon: Some(Icon::svg(material::hardware::laptop::regular()).into()),
            title: "No trusted devices".to_string(),
            description: (!empty.compact)
                .then(|| "Add a device for faster secure sign-in.".to_string()),
            action: Button(
                variant: ButtonVariant::Outline,
                size: ComponentSize::Sm,
                content: ButtonContent::new("Add device")
                    .leading_icon(Icon::svg(material::content::add::regular())),
                on_press: with_reducer!(cx.build, AddQualityDevice, add_quality_device),
            )
            .semantics_identifier("quality-gallery.device.add"),
        ))
}

fission_component! {
    pub(super) TrustedDevicesCard<GalleryState> { compact: bool }
    |card, _state, _cx| {} =>
        CardLayout::new()
            .content(CardContent::new(TrustedDevicesEmpty::new(card.compact)))
            .size(ComponentSize::Sm)
            .pattern(CardPattern::Plain)
}
