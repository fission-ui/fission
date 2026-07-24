use crate::components::result_line::ResultLine;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{ActionButton, Metric, PanelCard, ResponsiveGrid};
use crate::model::{
    nfc_uri_for_display, on_verify_with_barcode, on_verify_with_nfc, FieldInspectorState,
    VerifyWithBarcode, VerifyWithNfc,
};
use fission::prelude::*;

pub struct VerifyPanel;

impl From<VerifyPanel> for Widget {
    fn from(_: VerifyPanel) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let order = view.state().selected_order();
        let scan_barcode = with_reducer!(ctx, VerifyWithBarcode, on_verify_with_barcode);
        let scan_nfc = with_reducer!(ctx, VerifyWithNfc, on_verify_with_nfc);
        let spacing = &view.env().theme.tokens.spacing;

        PanelCard::new(Column {
            gap: Some(spacing.m),
            children: widgets![
                SectionHeader {
                    title: "Verify the asset",
                    body: "Use the field label and the embedded service tag to prove the technician is inspecting the right physical unit.",
                },
                ResponsiveGrid::new(widgets![
                    Metric::new("Expected barcode", order.asset.expected_barcode),
                    Metric::new("Expected NFC", order.asset.expected_nfc_uri),
                ]),
                Row {
                    gap: Some(spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        ActionButton::new(
                            "field-inspector.verify.barcode",
                            "Scan barcode",
                            scan_barcode,
                            ButtonVariant::Primary,
                        ),
                        ActionButton::new(
                            "field-inspector.verify.nfc",
                            "Tap NFC tag",
                            scan_nfc,
                            ButtonVariant::SecondaryColor,
                        ),
                    ],
                    ..Default::default()
                },
                ResultLine {
                    label: "Barcode result",
                    value: view
                        .state()
                        .scanned_barcode
                        .as_ref()
                        .and_then(|result| result.items.first())
                        .map(|item| item.value.clone()),
                    matched: view.state().asset_barcode_matches(),
                },
                ResultLine {
                    label: "NFC result",
                    value: view
                        .state()
                        .scanned_nfc
                        .as_ref()
                        .and_then(nfc_uri_for_display),
                    matched: view.state().asset_nfc_matches(),
                },
            ],
            ..Default::default()
        })
        .into()
    }
}
