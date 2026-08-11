use std::ffi::c_char;
use std::fmt;

use crate::ffi;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidArgument,
    InvalidHandle,
    InvalidState,
    Unsupported,
    WrongThread,
    SurfaceLost,
    ContextLost,
    DeviceLost,
    OutOfMemory,
    AbiMismatch,
    Internal,
    Unknown(u32),
}

impl ErrorKind {
    fn from_status(status: ffi::Status) -> Self {
        match status {
            ffi::STATUS_INVALID_ARGUMENT => Self::InvalidArgument,
            ffi::STATUS_INVALID_HANDLE => Self::InvalidHandle,
            ffi::STATUS_INVALID_STATE => Self::InvalidState,
            ffi::STATUS_UNSUPPORTED => Self::Unsupported,
            ffi::STATUS_WRONG_THREAD => Self::WrongThread,
            ffi::STATUS_SURFACE_LOST => Self::SurfaceLost,
            ffi::STATUS_CONTEXT_LOST => Self::ContextLost,
            ffi::STATUS_DEVICE_LOST => Self::DeviceLost,
            ffi::STATUS_OUT_OF_MEMORY => Self::OutOfMemory,
            ffi::STATUS_ABI_MISMATCH => Self::AbiMismatch,
            ffi::STATUS_INTERNAL => Self::Internal,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub sequence: u64,
    pub operation: String,
    pub message: String,
}

impl Error {
    pub(crate) fn from_ffi(status: ffi::Status, raw: &ffi::Error) -> Self {
        let code = if raw.code == ffi::STATUS_OK {
            status
        } else {
            raw.code
        };
        Self {
            kind: ErrorKind::from_status(code),
            sequence: raw.sequence,
            operation: decode_c_buffer(&raw.operation),
            message: decode_c_buffer(&raw.message),
        }
    }

    pub(crate) fn local(kind: ErrorKind, operation: &str, message: impl Into<String>) -> Self {
        Self {
            kind,
            sequence: 0,
            operation: operation.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Skia bridge {:?} error in {}: {}",
            self.kind, self.operation, self.message
        )
    }
}

impl std::error::Error for Error {}

pub(crate) fn status_result(status: ffi::Status, raw: &ffi::Error) -> Result<()> {
    if status == ffi::STATUS_OK {
        Ok(())
    } else {
        Err(Error::from_ffi(status, raw))
    }
}

pub(crate) fn decode_c_buffer(buffer: &[c_char]) -> String {
    let length = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    let bytes: Vec<u8> = buffer[..length].iter().map(|byte| *byte as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
