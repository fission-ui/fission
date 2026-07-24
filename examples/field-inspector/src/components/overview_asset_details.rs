use crate::components::ui::{BodyText, MutedText};
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct OverviewAssetDetails;

impl From<OverviewAssetDetails> for Widget {
    fn from(_: OverviewAssetDetails) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let order = view.state().selected_order();
        let tokens = &view.env().theme.tokens;

        Column {
            gap: Some(tokens.spacing.xs),
            flex_grow: 1.0,
            children: widgets![
                Text::new(order.asset.name)
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold),
                MutedText::new(order.asset.kind),
                BodyText::new(format!(
                    "Expected barcode {} and NFC {}",
                    order.asset.expected_barcode, order.asset.expected_nfc_uri
                )),
            ],
            ..Default::default()
        }
        .into()
    }
}
