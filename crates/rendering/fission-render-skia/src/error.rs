use fission_render::backend::{BackendError, BackendOperation};
use fission_render::diagnostics::{BackendDiagnostic, DiagnosticCategory, DiagnosticSeverity};

use crate::api::{ApiError, ApiErrorKind};
use crate::thread_owner::WrongThread;

pub(crate) fn api_error(operation: BackendOperation, error: ApiError) -> BackendError {
    let (category, stable_code) = match error.kind {
        ApiErrorKind::InvalidArgument => (DiagnosticCategory::Surface, "skia-invalid-argument"),
        ApiErrorKind::Unsupported => (DiagnosticCategory::Capability, "skia-unsupported"),
        ApiErrorKind::WrongThread => (DiagnosticCategory::Lifecycle, "skia-wrong-thread"),
        ApiErrorKind::SurfaceLost => (DiagnosticCategory::Surface, "skia-surface-lost"),
        ApiErrorKind::DeviceLost => (DiagnosticCategory::Device, "skia-device-lost"),
        ApiErrorKind::OutOfMemory => (DiagnosticCategory::Device, "skia-out-of-memory"),
        ApiErrorKind::Internal => (DiagnosticCategory::Device, "skia-internal-error"),
    };
    let message = format!(
        "Skia {} failed [{}]: {}",
        error.operation, error.code, error.message
    );
    BackendError::new(operation, stable_code, message.clone()).with_diagnostic(BackendDiagnostic {
        severity: DiagnosticSeverity::Error,
        category,
        code: stable_code.into(),
        message,
        provenance: None,
    })
}

pub(crate) fn wrong_thread(operation: BackendOperation, error: WrongThread) -> BackendError {
    let message = error.to_string();
    BackendError::new(operation, "skia-wrong-thread", message.clone()).with_diagnostic(
        BackendDiagnostic {
            severity: DiagnosticSeverity::Error,
            category: DiagnosticCategory::Lifecycle,
            code: "skia-wrong-thread".into(),
            message,
            provenance: None,
        },
    )
}

pub(crate) fn contract_error(
    operation: BackendOperation,
    code: &'static str,
    category: DiagnosticCategory,
    message: impl Into<String>,
) -> BackendError {
    let message = message.into();
    BackendError::new(operation, code, message.clone()).with_diagnostic(BackendDiagnostic {
        severity: DiagnosticSeverity::Error,
        category,
        code: code.into(),
        message,
        provenance: None,
    })
}
