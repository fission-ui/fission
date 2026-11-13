use fission_core::{
    collect_data_stream, FileWriteSource, PrintError, PrintOutcome, PrintPdfRequest,
    PrintPdfResult, PRINT_PDF,
};
use fission_shell::async_host::AsyncRegistry;
use std::ffi::{c_void, CStr};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use block::ConcreteBlock;
use dispatch::Queue;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

#[link(name = "UIKit", kind = "framework")]
extern "C" {}

pub(crate) fn register_ios_print_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PRINT_PDF,
        |request: PrintPdfRequest, ctx| async move {
            request.validate()?;
            if request.copies != 1 || !request.page_ranges.is_empty() {
                return Err(PrintError::unsupported(
                    "iOS print page and copy settings (set these in the system print sheet)",
                ));
            }
            let pdf = match request.pdf {
                FileWriteSource::Bytes(bytes) => bytes,
                FileWriteSource::Stream(id) => {
                    let stream = ctx.open_data_stream(id).map_err(|error| {
                        PrintError::new("stream_open_failed", error.to_string())
                    })?;
                    collect_data_stream(stream)
                        .await
                        .map_err(|error| PrintError::new("stream_read_failed", error.to_string()))?
                        .to_vec()
                }
            };
            if !pdf.starts_with(b"%PDF-") {
                return Err(PrintError::new(
                    "invalid_pdf",
                    "the print capability requires a PDF document beginning with `%PDF-`",
                ));
            }
            let job_name = request
                .job_name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "Fission document".into());
            let outcome = present_print_sheet(pdf, job_name)?;
            Ok(PrintPdfResult {
                outcome,
                job_id: None,
            })
        },
    );
}

fn present_print_sheet(pdf: Vec<u8>, job_name: String) -> Result<PrintOutcome, PrintError> {
    let state = Arc::new((Mutex::new(None), Condvar::new()));
    let state_on_main = state.clone();
    Queue::main().exec_async(move || unsafe {
        let controller: *mut Object =
            msg_send![class!(UIPrintInteractionController), sharedPrintController];
        if controller.is_null() {
            finish(
                &state_on_main,
                Err(PrintError::new(
                    "print_sheet_unavailable",
                    "iOS did not provide a print interaction controller",
                )),
            );
            return;
        }

        let data: *mut Object = msg_send![
            class!(NSData),
            dataWithBytes: pdf.as_ptr() as *const c_void
            length: pdf.len()
        ];
        let _: () = msg_send![controller, setPrintingItem: data];
        let _: () = msg_send![controller, setShowsNumberOfCopies: true];
        let _: () = msg_send![controller, setShowsPageRange: true];
        let print_info: *mut Object = msg_send![class!(UIPrintInfo), printInfo];
        let job_name_value = ns_string(&job_name);
        let _: () = msg_send![print_info, setJobName: job_name_value];
        let _: () = msg_send![job_name_value, release];
        let _: () = msg_send![controller, setPrintInfo: print_info];

        let callback_state = state_on_main.clone();
        let callback = ConcreteBlock::new(
            move |_controller: *mut Object, completed: bool, error: *mut Object| {
                if !error.is_null() {
                    let description: *mut Object = msg_send![error, localizedDescription];
                    let message = ns_string_to_string(description)
                        .unwrap_or_else(|| "iOS print sheet failed".into());
                    finish(
                        &callback_state,
                        Err(PrintError::new("print_failed", message)),
                    );
                } else if completed {
                    finish(&callback_state, Ok(PrintOutcome::Submitted));
                } else {
                    finish(&callback_state, Ok(PrintOutcome::Cancelled));
                }
            },
        )
        .copy();

        let presented: bool = msg_send![
            controller,
            presentAnimated: true
            completionHandler: &*callback
        ];
        if !presented {
            finish(
                &state_on_main,
                Err(PrintError::new(
                    "print_sheet_unavailable",
                    "iOS could not present the system print sheet",
                )),
            );
        }
    });

    let (lock, wake) = &*state;
    let result = lock.lock().map_err(|_| {
        PrintError::new(
            "print_callback_failed",
            "iOS print callback lock was poisoned",
        )
    })?;
    let (mut result, timeout) = wake
        .wait_timeout_while(result, Duration::from_secs(120), |result| result.is_none())
        .map_err(|_| {
            PrintError::new(
                "print_callback_failed",
                "iOS print callback lock was poisoned",
            )
        })?;
    if timeout.timed_out() && result.is_none() {
        return Err(PrintError::new(
            "print_timeout",
            "iOS did not finish the print sheet within two minutes",
        ));
    }
    result.take().unwrap_or_else(|| {
        Err(PrintError::new(
            "print_failed",
            "iOS print sheet returned no result",
        ))
    })
}

fn finish(
    state: &Arc<(Mutex<Option<Result<PrintOutcome, PrintError>>>, Condvar)>,
    result: Result<PrintOutcome, PrintError>,
) {
    let (lock, wake) = &**state;
    if let Ok(mut slot) = lock.lock() {
        if slot.is_none() {
            *slot = Some(result);
            wake.notify_one();
        }
    }
}

unsafe fn ns_string(value: &str) -> *mut Object {
    let string: *mut Object = msg_send![class!(NSString), alloc];
    msg_send![
        string,
        initWithBytes: value.as_ptr() as *const c_void
        length: value.len()
        encoding: 4usize
    ]
}

unsafe fn ns_string_to_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let pointer: *const i8 = msg_send![value, UTF8String];
    if pointer.is_null() {
        return None;
    }
    Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
}
