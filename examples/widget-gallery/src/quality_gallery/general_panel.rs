use fission::prelude::*;

use super::account_card::AccountDetailsCard;
use super::action_card::ActionHierarchyCard;
use super::empty_card::TrustedDevicesCard;
use crate::GalleryState;

fission_component! {
    pub(super) QualityGeneralPanel<GalleryState> { compact: bool }
    |panel, _state, _cx| {} =>
        if panel.compact {
            Column(
                gap: 12.0_f32,
                children: [
                    AccountDetailsCard(true),
                    Row(
                        gap: 12.0_f32,
                        align_items: fission::op::AlignItems::Stretch,
                        children: [
                            Container(ActionHierarchyCard(true))
                                .width_length(Length::percent(50.0))
                                .min_height(211.25)
                                .flex_grow(1.0),
                            Container(TrustedDevicesCard(true))
                                .width_length(Length::percent(50.0))
                                .min_height(211.25)
                                .flex_grow(1.0),
                        ],
                    ),
                ],
            )
        } else {
            Row(
                gap: 16.0_f32,
                align_items: fission::op::AlignItems::Stretch,
                children: [
                    Container(AccountDetailsCard(false))
                        .width_length(Length::percent(64.0))
                        .flex_grow(1.55),
                    Container(Column(
                        gap: 16.0_f32,
                        children: [
                            Container(ActionHierarchyCard(false)).min_height(185.0),
                            Container(TrustedDevicesCard(false)).min_height(250.5),
                        ],
                    ))
                    .width_length(Length::percent(35.0))
                    .flex_grow(0.85),
                ],
            )
        }
}
