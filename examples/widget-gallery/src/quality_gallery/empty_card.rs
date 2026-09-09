use fission::prelude::*;

use crate::GalleryState;

#[fission_reducer(AddQualityDevice)]
fn add_quality_device(_state: &mut GalleryState) {}

pub(super) struct TrustedDevicesCard {
    pub compact: bool,
}

impl From<TrustedDevicesCard> for Widget {
    fn from(card: TrustedDevicesCard) -> Self {
        let (ctx, _) = fission::build::current::<GalleryState>();

        CardLayout::new()
            .content(CardContent::new(Container::new(EmptyState {
                icon: Some(Icon::svg(material::hardware::laptop::regular()).into()),
                title: "No trusted devices".into(),
                description: (!card.compact)
                    .then(|| "Add a device for faster secure sign-in.".into()),
                action: Some(
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ComponentSize::Sm,
                        content: Some(
                            ButtonContent::new("Add device")
                                .leading_icon(Icon::svg(material::content::add::regular())),
                        ),
                        on_press: Some(with_reducer!(ctx, AddQualityDevice, add_quality_device)),
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.device.add")
                    .into(),
                ),
            })))
            .size(ComponentSize::Sm)
            .pattern(CardPattern::Plain)
            .into()
    }
}
