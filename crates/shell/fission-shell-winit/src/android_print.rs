use crate::android_capabilities::{app_class, AndroidHostContext};
use fission_core::{
    collect_data_stream, FileWriteSource, PrintError, PrintOutcome, PrintPdfRequest,
    PrintPdfResult, PRINT_PDF,
};
use fission_shell::async_host::AsyncRegistry;
use jni::objects::{JObject, JString, JValue};

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
                let pdf = match request.pdf {
                    FileWriteSource::Bytes(bytes) => bytes,
                    FileWriteSource::Stream(id) => {
                        let stream = ctx.open_data_stream(id).map_err(|error| {
                            PrintError::new("stream_open_failed", error.to_string())
                        })?;
                        collect_data_stream(stream)
                            .await
                            .map_err(|error| {
                                PrintError::new("stream_read_failed", error.to_string())
                            })?
                            .to_vec()
                    }
                };
                if !pdf.starts_with(b"%PDF-") {
                    return Err(PrintError::new(
                        "invalid_pdf",
                        "the print capability requires a PDF document beginning with `%PDF-`",
                    ));
                }

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
                            let pdf = env.byte_array_from_slice(&pdf)?;
                            let pdf_object = JObject::from(pdf);
                            let name = env.new_string(&name)?;
                            let name_object = JObject::from(name);
                            let result = env.call_static_method(
                            helper,
                            "printPdf",
                            "(Landroid/app/Activity;[BLjava/lang/String;)Ljava/lang/String;",
                            &[
                                JValue::Object(activity),
                                JValue::Object(&pdf_object),
                                JValue::Object(&name_object),
                            ],
                        )?.l()?;
                            let result = JString::from(result);
                            Ok(env.get_string(&result)?.to_string_lossy().into_owned())
                        })
                        .map_err(|message| PrintError::new("print_failed", message))?;

                Ok(PrintPdfResult {
                    outcome: PrintOutcome::Presented,
                    job_id: Some(job_id),
                })
            }
        },
    );
}
