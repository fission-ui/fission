use crate::components::ui::{BodyText, MutedText};
use crate::model::FieldInspectorState;
use fission::prelude::*;
fission_component! {
    pub OverviewAssetDetails<FieldInspectorState> {}
    |_details, state, cx| {
        let order = state.selected_order();
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.xs,
            flex_grow: 1.0,
            children: [
                Text(order.asset.name).size(tokens.typography.font_size_lg).weight(tokens.typography.font_weight_bold),
                MutedText(order.asset.kind.to_string()),
                BodyText(format!("Expected barcode {} and NFC {}", order.asset.expected_barcode, order.asset.expected_nfc_uri)),
            ],
        )
}
