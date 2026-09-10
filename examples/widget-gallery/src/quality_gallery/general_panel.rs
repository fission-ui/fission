use fission::prelude::*;

use super::account_card::AccountDetailsCard;
use super::action_card::ActionHierarchyCard;
use super::empty_card::TrustedDevicesCard;

pub(super) struct QualityGeneralPanel {
    pub compact: bool,
}

impl From<QualityGeneralPanel> for Widget {
    fn from(panel: QualityGeneralPanel) -> Self {
        if panel.compact {
            return Column {
                gap: Some(12.0),
                children: widgets![
                    AccountDetailsCard { compact: true },
                    Row {
                        gap: Some(12.0),
                        align_items: fission::op::AlignItems::Stretch,
                        children: widgets![
                            Container::new(ActionHierarchyCard { compact: true })
                                .width_length(Length::percent(50.0))
                                .min_height(211.25)
                                .flex_grow(1.0),
                            Container::new(TrustedDevicesCard { compact: true })
                                .width_length(Length::percent(50.0))
                                .min_height(211.25)
                                .flex_grow(1.0),
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }
            .into();
        }

        Row {
            gap: Some(16.0),
            align_items: fission::op::AlignItems::Stretch,
            children: widgets![
                Container::new(AccountDetailsCard { compact: false },)
                    .width_length(Length::percent(64.0))
                    .flex_grow(1.55),
                Container::new(Column {
                    gap: Some(16.0),
                    children: widgets![
                        Container::new(ActionHierarchyCard { compact: false }).min_height(185.0),
                        Container::new(TrustedDevicesCard { compact: false }).min_height(250.5),
                    ],
                    ..Default::default()
                })
                .width_length(Length::percent(35.0))
                .flex_grow(0.85),
            ],
            ..Default::default()
        }
        .into()
    }
}
