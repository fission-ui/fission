use fission_core::{
    collect_data_stream, CapabilityCtx, FileWriteSource, PrintError, PrintOutcome, PrintPageRange,
    PrintPdfRequest, PrintPdfResult, PRINT_PDF,
};
use fission_shell::async_host::AsyncRegistry;
use std::io::Write;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn register_native_print_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PRINT_PDF,
        |request: PrintPdfRequest, ctx: CapabilityCtx| async move {
            request.validate()?;
            let pdf = match request.pdf.clone() {
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
            validate_pdf(&pdf)?;
            submit_pdf(request, pdf)
        },
    );
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) fn register_native_print_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PRINT_PDF,
        |_request: PrintPdfRequest, _ctx| async move {
            Err::<PrintPdfResult, _>(PrintError::unsupported("print_pdf"))
        },
    );
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn submit_pdf(request: PrintPdfRequest, pdf: Vec<u8>) -> Result<PrintPdfResult, PrintError> {
    use std::path::Path;

    submit_pdf_with_programs(&request, &pdf, &[Path::new("lp"), Path::new("lpr")])
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn submit_pdf_with_programs(
    request: &PrintPdfRequest,
    pdf: &[u8],
    programs: &[&std::path::Path],
) -> Result<PrintPdfResult, PrintError> {
    use std::io::ErrorKind;
    use std::process::{Command, Stdio};

    for path in programs {
        let program = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("print service");
        let mut command = Command::new(path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match program {
            "lp" => {
                if let Some(name) = request.job_name.as_deref().filter(|name| !name.is_empty()) {
                    command.args(["-t", name]);
                }
                if request.copies > 1 {
                    command.arg("-n").arg(request.copies.to_string());
                }
            }
            _ => {
                if let Some(name) = request.job_name.as_deref().filter(|name| !name.is_empty()) {
                    command.args(["-J", name]);
                }
                if request.copies > 1 {
                    command.arg("-#").arg(request.copies.to_string());
                }
            }
        }
        if !request.page_ranges.is_empty() {
            command
                .arg("-o")
                .arg(format!("page-ranges={}", page_ranges(&request.page_ranges)));
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(PrintError::new(
                    "print_service_start_failed",
                    format!("could not start {program}: {error}"),
                ));
            }
        };
        let mut stdin = child.stdin.take().ok_or_else(|| {
            PrintError::new(
                "print_service_start_failed",
                "print service stdin was unavailable",
            )
        })?;
        stdin.write_all(pdf).map_err(|error| {
            PrintError::new(
                "print_service_write_failed",
                format!("could not send the PDF to {program}: {error}"),
            )
        })?;
        drop(stdin);
        let output = child.wait_with_output().map_err(|error| {
            PrintError::new(
                "print_service_wait_failed",
                format!("could not read the {program} result: {error}"),
            )
        })?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(PrintError::new(
                "print_submission_failed",
                if detail.is_empty() {
                    format!("{program} rejected the print job")
                } else {
                    format!("{program} rejected the print job: {detail}")
                },
            ));
        }
        return Ok(PrintPdfResult {
            outcome: PrintOutcome::Submitted,
            job_id: parse_lp_job_id(&output.stdout),
        });
    }

    Err(PrintError::unsupported(
        "print_pdf (install and configure CUPS with `lp` or `lpr`)",
    ))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn page_ranges(ranges: &[PrintPageRange]) -> String {
    ranges
        .iter()
        .map(|range| {
            if range.first_page == range.last_page {
                range.first_page.to_string()
            } else {
                format!("{}-{}", range.first_page, range.last_page)
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn parse_lp_job_id(stdout: &[u8]) -> Option<String> {
    let output = String::from_utf8_lossy(stdout);
    let rest = output.split_once("request id is ")?.1;
    let id = rest.split_whitespace().next()?;
    (!id.is_empty()).then(|| id.to_string())
}

#[cfg(target_os = "windows")]
fn submit_pdf(request: PrintPdfRequest, pdf: Vec<u8>) -> Result<PrintPdfResult, PrintError> {
    use std::os::windows::ffi::OsStrExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use windows::Win32::{
        Foundation::HWND,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };

    if request.copies != 1 || !request.page_ranges.is_empty() {
        return Err(PrintError::unsupported(
            "page and copy overrides for the Windows registered PDF print handler",
        ));
    }

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fission-print-{}-{nonce}-{}.pdf",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            PrintError::new(
                "print_document_write_failed",
                format!("could not stage the temporary PDF for printing: {error}"),
            )
        })?;
    if let Err(error) = file.write_all(&pdf) {
        let _ = std::fs::remove_file(&path);
        return Err(PrintError::new(
            "print_document_write_failed",
            format!("could not write the temporary PDF for printing: {error}"),
        ));
    }
    drop(file);

    let verb: Vec<u16> = "print".encode_utf16().chain(Some(0)).collect();
    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            HWND::default(),
            windows::core::PCWSTR(verb.as_ptr()),
            windows::core::PCWSTR(path_wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let status = result.0 as usize;
    if status <= 32 {
        let _ = std::fs::remove_file(&path);
        return Err(PrintError::new(
            "print_submission_failed",
            format!("Windows could not start the registered PDF print handler ({status})"),
        ));
    }

    // ShellExecute hands the path to the registered PDF handler and can return
    // before that process has opened it. Keep the staged file available while
    // the handler starts; Windows also clears the temp directory periodically.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60 * 60));
        let _ = std::fs::remove_file(path);
    });

    Ok(PrintPdfResult {
        outcome: PrintOutcome::Submitted,
        job_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rejects_invalid_copy_and_page_ranges() {
        assert_eq!(
            PrintPdfRequest {
                copies: 0,
                ..Default::default()
            }
            .validate()
            .unwrap_err()
            .code,
            "invalid_copies"
        );
        assert_eq!(
            PrintPdfRequest {
                page_ranges: vec![PrintPageRange {
                    first_page: 3,
                    last_page: 2,
                }],
                ..Default::default()
            }
            .validate()
            .unwrap_err()
            .code,
            "invalid_page_range"
        );
    }

    #[test]
    fn validation_rejects_non_pdf_input() {
        assert_eq!(validate_pdf(b"not a pdf").unwrap_err().code, "invalid_pdf");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn page_ranges_format_as_cups_page_ranges() {
        assert_eq!(
            page_ranges(&[
                PrintPageRange {
                    first_page: 1,
                    last_page: 1,
                },
                PrintPageRange {
                    first_page: 3,
                    last_page: 6,
                },
            ]),
            "1,3-6"
        );
    }

    #[cfg(unix)]
    #[test]
    fn submits_pdf_bytes_and_requested_settings_to_a_cups_command() {
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "fission-print-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let fake_lp = root.join("lp");
        let captured_pdf = root.join("captured.pdf");
        let captured_args = root.join("args.txt");
        std::fs::write(
            &fake_lp,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > '{}'\nprintf 'request id is fission-test-17 (1 file)\\n'\n",
                captured_args.display(),
                captured_pdf.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&fake_lp).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_lp, permissions).unwrap();

        let pdf = b"%PDF-1.7 test bytes";
        let request = PrintPdfRequest {
            job_name: Some("Quarterly report".into()),
            copies: 2,
            page_ranges: vec![
                PrintPageRange {
                    first_page: 1,
                    last_page: 1,
                },
                PrintPageRange {
                    first_page: 3,
                    last_page: 5,
                },
            ],
            ..Default::default()
        };
        let result = submit_pdf_with_programs(&request, pdf, &[Path::new(&fake_lp)]).unwrap();

        assert_eq!(result.outcome, PrintOutcome::Submitted);
        assert_eq!(result.job_id.as_deref(), Some("fission-test-17"));
        assert_eq!(std::fs::read(&captured_pdf).unwrap(), pdf);
        let args = std::fs::read_to_string(&captured_args).unwrap();
        assert!(args.contains("-t\nQuarterly report\n"));
        assert!(args.contains("-n\n2\n"));
        assert!(args.contains("page-ranges=1,3-5"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
