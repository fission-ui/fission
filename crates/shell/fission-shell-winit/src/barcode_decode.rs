use fission_core::{
    BarcodeFormat, BarcodePoint, BarcodeScanResult, BarcodeScanResults, BarcodeScannerError,
};

pub(crate) fn decode_barcode_bytes(
    bytes: &[u8],
    formats: &[BarcodeFormat],
) -> Result<BarcodeScanResults, BarcodeScannerError> {
    if bytes.is_empty() {
        return Err(BarcodeScannerError::new(
            "empty_image",
            "barcode image bytes were empty",
        ));
    }

    let mut items = Vec::new();
    if formats.is_empty() {
        items.push(decode_one_barcode(bytes, None)?);
    } else {
        let mut last_error = None;
        for format in formats {
            match decode_one_barcode(bytes, rxing_barcode_format(format)) {
                Ok(result) => items.push(result),
                Err(error) => last_error = Some(error),
            }
        }
        if items.is_empty() {
            return Err(last_error.unwrap_or_else(|| {
                BarcodeScannerError::new("not_found", "no barcode was found in the image")
            }));
        }
    }

    Ok(BarcodeScanResults { items })
}

fn decode_one_barcode(
    bytes: &[u8],
    format: Option<rxing::BarcodeFormat>,
) -> Result<BarcodeScanResult, BarcodeScannerError> {
    let result = rxing::helpers::detect_in_buffer(bytes, format)
        .map_err(|error| BarcodeScannerError::new("decode_failed", error.to_string()))?;
    Ok(BarcodeScanResult {
        value: result.getText().to_string(),
        format: fission_barcode_format(result.getBarcodeFormat()),
        raw_bytes: result.getRawBytes().to_vec(),
        bounds: result
            .getPoints()
            .iter()
            .map(|point| BarcodePoint {
                x: point.x.round() as i32,
                y: point.y.round() as i32,
            })
            .collect(),
        symbology_identifier: None,
    })
}

fn rxing_barcode_format(format: &BarcodeFormat) -> Option<rxing::BarcodeFormat> {
    Some(match format {
        BarcodeFormat::QrCode => rxing::BarcodeFormat::QR_CODE,
        BarcodeFormat::Aztec => rxing::BarcodeFormat::AZTEC,
        BarcodeFormat::DataMatrix => rxing::BarcodeFormat::DATA_MATRIX,
        BarcodeFormat::Ean13 => rxing::BarcodeFormat::EAN_13,
        BarcodeFormat::Ean8 => rxing::BarcodeFormat::EAN_8,
        BarcodeFormat::Code128 => rxing::BarcodeFormat::CODE_128,
        BarcodeFormat::Code39 => rxing::BarcodeFormat::CODE_39,
        BarcodeFormat::Code93 => rxing::BarcodeFormat::CODE_93,
        BarcodeFormat::Codabar => rxing::BarcodeFormat::CODABAR,
        BarcodeFormat::Itf => rxing::BarcodeFormat::ITF,
        BarcodeFormat::Pdf417 => rxing::BarcodeFormat::PDF_417,
        BarcodeFormat::UpcA => rxing::BarcodeFormat::UPC_A,
        BarcodeFormat::UpcE => rxing::BarcodeFormat::UPC_E,
        BarcodeFormat::MaxiCode => rxing::BarcodeFormat::MAXICODE,
        BarcodeFormat::Rss14 => rxing::BarcodeFormat::RSS_14,
        BarcodeFormat::RssExpanded => rxing::BarcodeFormat::RSS_EXPANDED,
        BarcodeFormat::Other(_) => return None,
    })
}

fn fission_barcode_format(format: &rxing::BarcodeFormat) -> BarcodeFormat {
    match format {
        rxing::BarcodeFormat::QR_CODE => BarcodeFormat::QrCode,
        rxing::BarcodeFormat::AZTEC => BarcodeFormat::Aztec,
        rxing::BarcodeFormat::DATA_MATRIX => BarcodeFormat::DataMatrix,
        rxing::BarcodeFormat::EAN_13 => BarcodeFormat::Ean13,
        rxing::BarcodeFormat::EAN_8 => BarcodeFormat::Ean8,
        rxing::BarcodeFormat::CODE_128 => BarcodeFormat::Code128,
        rxing::BarcodeFormat::CODE_39 => BarcodeFormat::Code39,
        rxing::BarcodeFormat::CODE_93 => BarcodeFormat::Code93,
        rxing::BarcodeFormat::CODABAR => BarcodeFormat::Codabar,
        rxing::BarcodeFormat::ITF => BarcodeFormat::Itf,
        rxing::BarcodeFormat::PDF_417 => BarcodeFormat::Pdf417,
        rxing::BarcodeFormat::UPC_A => BarcodeFormat::UpcA,
        rxing::BarcodeFormat::UPC_E => BarcodeFormat::UpcE,
        rxing::BarcodeFormat::MAXICODE => BarcodeFormat::MaxiCode,
        rxing::BarcodeFormat::RSS_14 => BarcodeFormat::Rss14,
        rxing::BarcodeFormat::RSS_EXPANDED => BarcodeFormat::RssExpanded,
        other => BarcodeFormat::Other(format!("{other:?}")),
    }
}
