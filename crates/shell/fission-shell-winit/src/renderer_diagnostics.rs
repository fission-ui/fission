use fission_diagnostics::prelude as diag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RendererRequest {
    Auto,
    Vello,
    Skia,
    Software,
    WebGpuVello,
    Canvas2dSoftware,
    NativeVelloGpu,
    NativeVelloCpu,
    NativeSkiaRaster,
    NativeSoftware,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RendererTarget {
    Native,
    Web,
}

impl RendererRequest {
    pub(crate) fn from_env() -> Self {
        renderer_request_from_value(std::env::var("FISSION_RENDERER").ok().as_deref())
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Vello => "vello",
            Self::Skia => "skia",
            Self::Software => "software",
            Self::WebGpuVello => "webgpu-vello",
            Self::Canvas2dSoftware => "canvas2d-software",
            Self::NativeVelloGpu => "native-vello-gpu",
            Self::NativeVelloCpu => "native-vello-cpu",
            Self::NativeSkiaRaster => "native-skia-raster",
            Self::NativeSoftware => "native-software",
            Self::Invalid => "invalid",
        }
    }

    pub(crate) fn is_explicit_gpu(self) -> bool {
        matches!(self, Self::Vello | Self::WebGpuVello | Self::NativeVelloGpu)
    }

    pub(crate) fn for_target(self, target: RendererTarget) -> Result<Self, RendererSelectionError> {
        match (target, self) {
            (_, Self::Auto) => Ok(Self::Auto),
            (RendererTarget::Native, Self::Vello) => Ok(Self::NativeVelloGpu),
            (RendererTarget::Native, Self::Skia) => Ok(Self::NativeSkiaRaster),
            (RendererTarget::Native, Self::Software) => Ok(Self::NativeSoftware),
            (RendererTarget::Web, Self::Vello) => Ok(Self::WebGpuVello),
            (RendererTarget::Web, Self::Software) => Ok(Self::Canvas2dSoftware),
            (
                RendererTarget::Native,
                Self::NativeVelloGpu
                | Self::NativeVelloCpu
                | Self::NativeSkiaRaster
                | Self::NativeSoftware,
            )
            | (RendererTarget::Web, Self::WebGpuVello | Self::Canvas2dSoftware) => Ok(self),
            _ => Err(RendererSelectionError {
                request: self,
                target,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RendererSelectionError {
    pub(crate) request: RendererRequest,
    pub(crate) target: RendererTarget,
}

impl std::fmt::Display for RendererSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.request == RendererRequest::Invalid {
            return write!(
                formatter,
                "unsupported FISSION_RENDERER value; expected auto, vello, skia, software, webgpu-vello, canvas2d-software, native-vello-gpu, native-vello-cpu, native-skia-raster, or native-software"
            );
        }
        write!(
            formatter,
            "renderer request `{}` is unavailable for the {} target",
            self.request.as_str(),
            match self.target {
                RendererTarget::Native => "native",
                RendererTarget::Web => "web",
            }
        )
    }
}

impl std::error::Error for RendererSelectionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestedRendererInitializationError {
    pub(crate) request: RendererRequest,
    pub(crate) target: RendererTarget,
    pub(crate) details: String,
}

impl RequestedRendererInitializationError {
    pub(crate) fn new(
        request: RendererRequest,
        target: RendererTarget,
        details: impl Into<String>,
    ) -> Self {
        Self {
            request,
            target,
            details: details.into(),
        }
    }
}

impl std::fmt::Display for RequestedRendererInitializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "requested renderer `{}` could not initialize for the {} target: {}",
            self.request.as_str(),
            match self.target {
                RendererTarget::Native => "native",
                RendererTarget::Web => "web",
            },
            self.details
        )
    }
}

impl std::error::Error for RequestedRendererInitializationError {}

pub(crate) fn renderer_error_is_terminal(error: &anyhow::Error) -> bool {
    error.downcast_ref::<RendererSelectionError>().is_some()
        || error
            .downcast_ref::<RequestedRendererInitializationError>()
            .is_some()
}

pub(crate) fn renderer_request_from_value(value: Option<&str>) -> RendererRequest {
    let Some(value) = value else {
        return RendererRequest::Auto;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => RendererRequest::Auto,
        "webgpu" | "webgpu-vello" => RendererRequest::WebGpuVello,
        "canvas" | "canvas2d" | "canvas2d-software" | "software-canvas" => {
            RendererRequest::Canvas2dSoftware
        }
        "vello" | "vello-gpu" | "gpu" => RendererRequest::Vello,
        "skia" | "skia-raster" => RendererRequest::Skia,
        "native-vello" | "native-vello-gpu" => RendererRequest::NativeVelloGpu,
        "vello-cpu" | "native-vello-cpu" | "cpu-vello" => RendererRequest::NativeVelloCpu,
        "native-skia" | "native-skia-raster" => RendererRequest::NativeSkiaRaster,
        "software" => RendererRequest::Software,
        "native-software" => RendererRequest::NativeSoftware,
        _ => RendererRequest::Invalid,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct RendererReport {
    pub active: String,
    pub requested: String,
    pub backend: Option<String>,
    pub adapter: Option<String>,
    pub fallback_reason: Option<String>,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

impl RendererReport {
    pub(crate) fn new(
        active: impl Into<String>,
        requested: RendererRequest,
        backend: Option<String>,
        adapter: Option<String>,
        fallback_reason: Option<String>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        Self {
            active: active.into(),
            requested: requested.as_str().to_string(),
            backend,
            adapter,
            fallback_reason,
            width,
            height,
            scale_factor,
        }
    }

    pub(crate) fn concise_line(&self) -> String {
        let fallback = self
            .fallback_reason
            .as_deref()
            .map(|reason| format!(" fallback_reason={reason}"))
            .unwrap_or_default();
        let backend = self
            .backend
            .as_deref()
            .map(|backend| format!(" backend={backend}"))
            .unwrap_or_default();
        let adapter = self
            .adapter
            .as_deref()
            .map(|adapter| format!(" adapter={adapter}"))
            .unwrap_or_default();
        format!(
            "renderer: {} requested={}{}{} size={}x{} scale={:.2}{}",
            self.active,
            self.requested,
            backend,
            adapter,
            self.width,
            self.height,
            self.scale_factor,
            fallback
        )
    }
}

pub(crate) fn emit_renderer_report(report: &RendererReport) {
    eprintln!("fission-shell-winit: {}", report.concise_line());
    diag::emit(
        diag::DiagCategory::Raster,
        diag::DiagLevel::Info,
        diag::DiagEventKind::RendererSelected {
            active: report.active.clone(),
            requested: report.requested.clone(),
            backend: report.backend.clone(),
            adapter: report.adapter.clone(),
            fallback_reason: report.fallback_reason.clone(),
            width: report.width,
            height: report.height,
            scale_factor: report.scale_factor,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{
        renderer_error_is_terminal, renderer_request_from_value, RendererRequest,
        RendererSelectionError, RendererTarget, RequestedRendererInitializationError,
    };

    #[test]
    fn renderer_request_parses_known_values() {
        assert_eq!(renderer_request_from_value(None), RendererRequest::Auto);
        assert_eq!(
            renderer_request_from_value(Some("webgpu-vello")),
            RendererRequest::WebGpuVello
        );
        assert_eq!(
            renderer_request_from_value(Some("canvas2d")),
            RendererRequest::Canvas2dSoftware
        );
        assert_eq!(
            renderer_request_from_value(Some("native-vello-cpu")),
            RendererRequest::NativeVelloCpu
        );
        assert_eq!(
            renderer_request_from_value(Some("skia")),
            RendererRequest::Skia
        );
        assert_eq!(
            renderer_request_from_value(Some("native-skia-raster")),
            RendererRequest::NativeSkiaRaster
        );
    }

    #[test]
    fn renderer_request_unknown_is_invalid() {
        assert_eq!(
            renderer_request_from_value(Some("not-a-renderer")),
            RendererRequest::Invalid
        );
    }

    #[test]
    fn configuration_and_explicit_initialization_errors_are_terminal() {
        let selection = anyhow::Error::new(RendererSelectionError {
            request: RendererRequest::Invalid,
            target: RendererTarget::Native,
        });
        let initialization = anyhow::Error::new(RequestedRendererInitializationError::new(
            RendererRequest::NativeVelloGpu,
            RendererTarget::Native,
            "adapter rejected the request",
        ));

        assert!(renderer_error_is_terminal(&selection));
        assert!(renderer_error_is_terminal(&initialization));
        assert!(!renderer_error_is_terminal(&anyhow::anyhow!(
            "surface temporarily unavailable"
        )));
    }

    #[test]
    fn unset_and_explicit_auto_remain_auto() {
        assert_eq!(renderer_request_from_value(None), RendererRequest::Auto);
        assert_eq!(
            renderer_request_from_value(Some("auto")),
            RendererRequest::Auto
        );
    }

    #[test]
    fn renderer_target_rejects_known_but_unavailable_requests() {
        assert_eq!(
            RendererRequest::Canvas2dSoftware
                .for_target(RendererTarget::Native)
                .unwrap_err(),
            RendererSelectionError {
                request: RendererRequest::Canvas2dSoftware,
                target: RendererTarget::Native,
            }
        );
        assert_eq!(
            RendererRequest::NativeSoftware
                .for_target(RendererTarget::Web)
                .unwrap_err(),
            RendererSelectionError {
                request: RendererRequest::NativeSoftware,
                target: RendererTarget::Web,
            }
        );
        assert_eq!(
            RendererRequest::Auto
                .for_target(RendererTarget::Native)
                .unwrap(),
            RendererRequest::Auto
        );
        assert_eq!(
            RendererRequest::Vello
                .for_target(RendererTarget::Native)
                .unwrap(),
            RendererRequest::NativeVelloGpu
        );
        assert_eq!(
            RendererRequest::Vello
                .for_target(RendererTarget::Web)
                .unwrap(),
            RendererRequest::WebGpuVello
        );
        assert_eq!(
            RendererRequest::Software
                .for_target(RendererTarget::Native)
                .unwrap(),
            RendererRequest::NativeSoftware
        );
        assert_eq!(
            RendererRequest::Software
                .for_target(RendererTarget::Web)
                .unwrap(),
            RendererRequest::Canvas2dSoftware
        );
        assert_eq!(
            RendererRequest::Skia
                .for_target(RendererTarget::Native)
                .unwrap(),
            RendererRequest::NativeSkiaRaster
        );
        assert_eq!(
            RendererRequest::Skia
                .for_target(RendererTarget::Web)
                .unwrap_err(),
            RendererSelectionError {
                request: RendererRequest::Skia,
                target: RendererTarget::Web,
            }
        );
        assert_eq!(
            RendererRequest::Auto
                .for_target(RendererTarget::Web)
                .unwrap(),
            RendererRequest::Auto
        );
    }

    #[test]
    fn invalid_renderer_has_a_stable_actionable_diagnostic() {
        let error = RendererRequest::Invalid
            .for_target(RendererTarget::Native)
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "unsupported FISSION_RENDERER value; expected auto, vello, skia, software, webgpu-vello, canvas2d-software, native-vello-gpu, native-vello-cpu, native-skia-raster, or native-software"
        );
    }
}
