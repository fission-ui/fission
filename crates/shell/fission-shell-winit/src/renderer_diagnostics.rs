use fission_diagnostics::prelude as diag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RendererRequest {
    Auto,
    WebGpuVello,
    Canvas2dSoftware,
    NativeVelloGpu,
    NativeVelloCpu,
    NativeSoftware,
}

impl RendererRequest {
    pub(crate) fn from_env() -> Self {
        renderer_request_from_value(std::env::var("FISSION_RENDERER").ok().as_deref())
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::WebGpuVello => "webgpu-vello",
            Self::Canvas2dSoftware => "canvas2d-software",
            Self::NativeVelloGpu => "native-vello-gpu",
            Self::NativeVelloCpu => "native-vello-cpu",
            Self::NativeSoftware => "native-software",
        }
    }

    pub(crate) fn is_explicit_gpu(self) -> bool {
        matches!(self, Self::WebGpuVello | Self::NativeVelloGpu)
    }
}

pub(crate) fn renderer_request_from_value(value: Option<&str>) -> RendererRequest {
    let Some(value) = value else {
        return RendererRequest::Auto;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "webgpu" | "webgpu-vello" => RendererRequest::WebGpuVello,
        "canvas" | "canvas2d" | "canvas2d-software" | "software-canvas" => {
            RendererRequest::Canvas2dSoftware
        }
        "vello" | "vello-gpu" | "native-vello" | "native-vello-gpu" | "gpu" => {
            RendererRequest::NativeVelloGpu
        }
        "vello-cpu" | "native-vello-cpu" | "cpu-vello" => RendererRequest::NativeVelloCpu,
        "software" | "native-software" => RendererRequest::NativeSoftware,
        _ => RendererRequest::Auto,
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
    use super::{renderer_request_from_value, RendererRequest};

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
    }

    #[test]
    fn renderer_request_unknown_is_auto() {
        assert_eq!(
            renderer_request_from_value(Some("not-a-renderer")),
            RendererRequest::Auto
        );
    }
}
