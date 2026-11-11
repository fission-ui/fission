use fission_core::{
    collect_data_stream, CapabilityCtx, FileWriteSource, PrintError, PrintOutcome, PrintPdfRequest,
    PrintPdfResult, PRINT_PDF,
};
use fission_shell::async_host::AsyncRegistry;
use js_sys::{Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(inline_js = r#"
export function fissionOpenPrintWindow() {
  const popup = globalThis.open("about:blank", "_blank");
  if (!popup) {
    const error = new Error("The browser blocked the print window. Allow popups and try again.");
    error.name = "popup_blocked";
    throw error;
  }
  return popup;
}

export function fissionClosePrintWindow(popup) {
  if (popup && !popup.closed) popup.close();
}

export function fissionShowPdfPrintWindow(popup, bytes, title) {
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/pdf" }));
  return new Promise((resolve, reject) => {
    let settled = false;
    const checkClosed = setInterval(() => {
      if (popup.closed) finish("Cancelled");
    }, 100);
    const loadTimeout = setTimeout(() => {
      const error = new Error("The browser did not load the PDF print view.");
      error.name = "print_load_timeout";
      finish(null, error);
    }, 30000);
    const finish = (outcome, error) => {
      if (settled) return;
      settled = true;
      clearInterval(checkClosed);
      clearTimeout(loadTimeout);
      if (error) reject(error); else resolve(outcome);
    };

    popup.addEventListener("load", () => {
      if (popup.closed) return finish("Cancelled");
      try {
        popup.document.title = title || "Print PDF";
        popup.addEventListener("unload", () => URL.revokeObjectURL(url), { once: true });
        popup.focus();
        popup.print();
        // Browser print APIs do not report whether the user accepted or
        // cancelled the dialog, so report presentation rather than completion.
        finish("Presented");
      } catch (error) {
        finish(null, error);
      }
    }, { once: true });
    popup.location.href = url;
  });
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = fissionOpenPrintWindow)]
    fn open_print_window() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = fissionClosePrintWindow)]
    fn close_print_window(popup: &JsValue) -> Result<(), JsValue>;

    #[wasm_bindgen(catch, js_name = fissionShowPdfPrintWindow)]
    fn show_pdf_print_window(
        popup: &JsValue,
        bytes: &Uint8Array,
        title: &str,
    ) -> Result<Promise, JsValue>;
}

pub(crate) fn register_web_print_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PRINT_PDF,
        |request: PrintPdfRequest, ctx: CapabilityCtx| async move {
            request.validate()?;
            validate_browser_print_options(&request)?;

            // Open the surface before awaiting a potentially asynchronous data
            // stream. This keeps the popup request in the user-action task.
            let popup = open_print_window().map_err(js_error)?;
            let bytes = match request.pdf {
                FileWriteSource::Bytes(bytes) => bytes,
                FileWriteSource::Stream(id) => {
                    let stream = match ctx.open_data_stream(id) {
                        Ok(stream) => stream,
                        Err(error) => {
                            let _ = close_print_window(&popup);
                            return Err(PrintError::new("stream_open_failed", error.to_string()));
                        }
                    };
                    match collect_data_stream(stream).await {
                        Ok(bytes) => bytes.to_vec(),
                        Err(error) => {
                            let _ = close_print_window(&popup);
                            return Err(PrintError::new("stream_read_failed", error.to_string()));
                        }
                    }
                }
            };
            if let Err(error) = validate_pdf(&bytes) {
                let _ = close_print_window(&popup);
                return Err(error);
            }

            let bytes = Uint8Array::from(bytes.as_slice());
            let title = request.job_name.unwrap_or_else(|| "Print PDF".into());
            let promise = show_pdf_print_window(&popup, &bytes, &title).map_err(js_error)?;
            let outcome = JsFuture::from(promise).await.map_err(js_error)?;
            match outcome.as_string().as_deref() {
                Some("Presented") => Ok(PrintPdfResult {
                    outcome: PrintOutcome::Presented,
                    job_id: None,
                }),
                Some("Cancelled") => Ok(PrintPdfResult {
                    outcome: PrintOutcome::Cancelled,
                    job_id: None,
                }),
                _ => Err(PrintError::new(
                    "invalid_host_response",
                    "the browser returned an unknown PDF print outcome",
                )),
            }
        },
    );
}

fn validate_browser_print_options(request: &PrintPdfRequest) -> Result<(), PrintError> {
    if request.copies != 1 || !request.page_ranges.is_empty() {
        return Err(PrintError::unsupported(
            "browser print page and copy overrides (use the browser print dialog)",
        ));
    }
    Ok(())
}

fn validate_pdf(pdf: &[u8]) -> Result<(), PrintError> {
    if !pdf.starts_with(b"%PDF-") {
        return Err(PrintError::new(
            "invalid_pdf",
            "the print capability requires a PDF document beginning with `%PDF-`",
        ));
    }
    Ok(())
}

fn js_error(value: JsValue) -> PrintError {
    let name = Reflect::get(&value, &JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string());
    let message = Reflect::get(&value, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "browser PDF printing failed".into());
    let code = match name.as_deref() {
        Some("popup_blocked") => "popup_blocked",
        Some("print_load_timeout") => "print_load_timeout",
        Some("NotAllowedError") => "permission_denied",
        _ => "print_failed",
    };
    PrintError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_print_rejects_copy_and_page_overrides_it_cannot_apply() {
        let error = validate_browser_print_options(&PrintPdfRequest {
            copies: 2,
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(error.code, "unsupported");
    }
}
