use crate::components::result_line::ResultLine;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{ActionButton, Metric, PanelCard, ResponsiveGrid};
use crate::model::{
    nfc_uri_for_display, on_verify_with_barcode, on_verify_with_nfc, FieldInspectorState,
    VerifyWithBarcode, VerifyWithNfc,
};
use fission::prelude::*;
fission_component! {
    pub VerifyPanel<FieldInspectorState> {}
    |_panel, state, cx| {
        let order = state.selected_order();
        let scan_barcode = with_reducer!(cx.build, VerifyWithBarcode, on_verify_with_barcode);
        let scan_nfc = with_reducer!(cx.build, VerifyWithNfc, on_verify_with_nfc);
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        PanelCard(Column(
            gap: spacing.m,
            children: [
                SectionHeader("Verify the asset", "Use the field label and the embedded service tag to prove the technician is inspecting the right physical unit."),
                ResponsiveGrid(vec![
                    Metric::new("Expected barcode".to_string(), order.asset.expected_barcode.to_string()).into(),
                    Metric::new("Expected NFC".to_string(), order.asset.expected_nfc_uri.to_string()).into(),
                ], crate::components::ui::DEFAULT_ITEM_MIN_WIDTH),
                Row(gap: spacing.s, wrap: ir_op::FlexWrap::Wrap, children: [
                    ActionButton("field-inspector.verify.barcode".to_string(), "Scan barcode".to_string(), scan_barcode, ButtonVariant::Primary),
                    ActionButton("field-inspector.verify.nfc".to_string(), "Tap NFC tag".to_string(), scan_nfc, ButtonVariant::SecondaryColor),
                ]),
                ResultLine("Barcode result", state.scanned_barcode.as_ref().and_then(|result| result.items.first()).map(|item| item.value.clone()), state.asset_barcode_matches()),
                ResultLine("NFC result", state.scanned_nfc.as_ref().and_then(nfc_uri_for_display), state.asset_nfc_matches()),
            ],
        ))
}
