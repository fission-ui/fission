use crate::android_capabilities::{app_class, AndroidHostContext};
use fission_core::{
    CapabilityCtx, FileWriteSource, PrintError, PrintOutcome, PrintPdfRequest, PrintPdfResult,
    PRINT_PDF,
};
use fission_shell::async_host::AsyncRegistry;
use jni::objects::{JObject, JString, JValue};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn register_android_print_capability(
    async_registry: &mut AsyncRegistry,
    context: AndroidHostContext,
) {
    async_registry.register_operation_capability(
        PRINT_PDF,
        move |request: PrintPdfRequest, ctx| {
            let context = context.clone();
            async move {
                request.validate()?;
                if request.copies != 1 || !request.page_ranges.is_empty() {
                    return Err(PrintError::unsupported(
                        "Android print page and copy settings (set these in the system print sheet)",
                    ));
                }
                // A print adapter may request the document repeatedly. Stage it
                // once off the UI thread, without collecting/copying a stream
                // into Rust and Java heaps. The adapter owns cleanup onFinish.
                let pdf = stage_pdf(request.pdf, &ctx, &context).await?;

                let name = request
                    .job_name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| "Fission document".into());
                let job_id =
                    context
                        .with_env(|env, activity| {
                            let helper = app_class(
                                env,
                                activity,
                                "rs.fission.runtime.FissionAndroidCapabilities",
                            )?;
                            let pdf = env.new_string(pdf.path.to_string_lossy())?;
                            let pdf_object = JObject::from(pdf);
                            let name = env.new_string(&name)?;
                            let name_object = JObject::from(name);
                            let result = env.call_static_method(
                            helper,
                            "printPdf",
                            "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                            &[
                                JValue::Object(activity),
                                JValue::Object(&pdf_object),
                                JValue::Object(&name_object),
                            ],
                        )?.l()?;
                            let result = JString::from(result);
                            let job_id = env.get_string(&result)?.to_string_lossy().into_owned();
                            Ok(job_id)
                        })
                        .map_err(|message| PrintError::new("print_failed", message))?;
                pdf.hand_off();

                Ok(PrintPdfResult {
                    outcome: PrintOutcome::Presented,
                    job_id: Some(job_id),
                })
            }
        },
    );
}

struct StagedPdf {
    path: PathBuf,
    remove_on_drop: bool,
}

impl StagedPdf {
    fn hand_off(mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for StagedPdf {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn stage_pdf(
    source: FileWriteSource,
    ctx: &CapabilityCtx,
    host: &AndroidHostContext,
) -> Result<StagedPdf, PrintError> {
    use std::os::unix::fs::OpenOptionsExt;
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let directory = host
        .with_env(|env, activity| {
            let cache = env
                .call_method(activity, "getCacheDir", "()Ljava/io/File;", &[])?
                .l()?;
            let path = env
                .call_method(&cache, "getAbsolutePath", "()Ljava/lang/String;", &[])?
                .l()?;
            let path = JString::from(path);
            let value = env.get_string(&path)?.to_string_lossy().into_owned();
            Ok(PathBuf::from(value))
        })
        .map_err(|error| PrintError::new("print_cache_unavailable", error))?;
    let path = directory.join(format!(
        "fission-print-{}-{}.pdf",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(print_io)?;
    // Only own/remove a path after create_new succeeds.
    let staged = StagedPdf {
        path,
        remove_on_drop: true,
    };
    match source {
        FileWriteSource::Bytes(bytes) => file.write_all(&bytes).map_err(print_io)?,
        FileWriteSource::Stream(id) => {
            let mut stream = ctx
                .open_data_stream(id)
                .map_err(|error| PrintError::new("stream_open_failed", error.to_string()))?;
            while let Some(chunk) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
                let chunk = chunk
                    .map_err(|error| PrintError::new("stream_read_failed", error.to_string()))?;
                file.write_all(&chunk).map_err(print_io)?;
            }
        }
    }
    file.flush().map_err(print_io)?;
    file.seek(SeekFrom::Start(0)).map_err(print_io)?;
    let mut header = [0; 5];
    if file.read_exact(&mut header).is_err() || &header != b"%PDF-" {
        return Err(PrintError::new(
            "invalid_pdf",
            "the print capability requires a PDF document beginning with `%PDF-`",
        ));
    }
    Ok(staged)
}

fn print_io(error: std::io::Error) -> PrintError {
    PrintError::new("print_document_io_failed", error.to_string())
}
