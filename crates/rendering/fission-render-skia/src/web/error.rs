use std::fmt;

use crate::compiler::CompileError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebCompileError {
    Scene(CompileError),
    NativeResource(&'static str),
    InvalidGeometry(&'static str),
    CommandStream(fission_skia_sys::web::CommandStreamError),
}

impl fmt::Display for WebCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(error) => error.fmt(formatter),
            Self::NativeResource(kind) => write!(
                formatter,
                "the CanvasKit compiler received a native-only {kind} resource"
            ),
            Self::InvalidGeometry(field) => {
                write!(formatter, "the CanvasKit compiler received invalid {field}")
            }
            Self::CommandStream(error) => write!(
                formatter,
                "the CanvasKit command stream rejected compiled output: {error}"
            ),
        }
    }
}

impl std::error::Error for WebCompileError {}

impl From<CompileError> for WebCompileError {
    fn from(error: CompileError) -> Self {
        Self::Scene(error)
    }
}

impl From<fission_skia_sys::web::CommandStreamError> for WebCompileError {
    fn from(error: fission_skia_sys::web::CommandStreamError) -> Self {
        Self::CommandStream(error)
    }
}
