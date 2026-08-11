use std::fmt;

pub const PARAGRAPH_WIRE_VERSION: u16 = 1;
pub const MAX_PARAGRAPH_PACKET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_STRING_BYTES: usize = 1024 * 1024;
pub const MAX_STYLE_RUNS: usize = 65_536;
pub const MAX_INLINE_OBJECTS: usize = 65_536;
pub const MAX_FALLBACK_FAMILIES: usize = 4_096;
pub const MAX_FONT_RESOURCES: usize = 4_096;
pub const MAX_VARIATIONS: usize = 65_536;
pub const MAX_FEATURES: usize = 65_536;
pub const MAX_GEOMETRY_RECORDS: usize = 1_048_576;
pub const MAX_UNRESOLVED_CODEPOINTS: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParagraphWireError {
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    LengthMismatch,
    NonZeroReserved,
    InvalidFlags,
    InvalidEnum(&'static str),
    InvalidValue(&'static str),
    InvalidUtf8(&'static str),
    LimitExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
}

impl fmt::Display for ParagraphWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ParagraphWireError {}

pub(super) fn require_limit(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), ParagraphWireError> {
    if actual > maximum {
        Err(ParagraphWireError::LimitExceeded {
            field,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

pub(super) fn finite(value: f32, field: &'static str) -> Result<f32, ParagraphWireError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ParagraphWireError::InvalidValue(field))
}

pub(super) fn non_negative(value: f32, field: &'static str) -> Result<f32, ParagraphWireError> {
    let value = finite(value, field)?;
    (value >= 0.0)
        .then_some(value)
        .ok_or(ParagraphWireError::InvalidValue(field))
}

pub(super) fn positive(value: f32, field: &'static str) -> Result<f32, ParagraphWireError> {
    let value = finite(value, field)?;
    (value > 0.0)
        .then_some(value)
        .ok_or(ParagraphWireError::InvalidValue(field))
}

pub(super) fn checked_u32(value: usize, field: &'static str) -> Result<u32, ParagraphWireError> {
    u32::try_from(value).map_err(|_| ParagraphWireError::InvalidValue(field))
}

pub(super) fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn put_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8], ParagraphWireError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ParagraphWireError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ParagraphWireError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn u8(&mut self) -> Result<u8, ParagraphWireError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, ParagraphWireError> {
        let mut value = [0; 2];
        value.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(value))
    }

    pub(super) fn u32(&mut self) -> Result<u32, ParagraphWireError> {
        let mut value = [0; 4];
        value.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(value))
    }

    pub(super) fn u64(&mut self) -> Result<u64, ParagraphWireError> {
        let mut value = [0; 8];
        value.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(value))
    }

    pub(super) fn f32(&mut self) -> Result<f32, ParagraphWireError> {
        let mut value = [0; 4];
        value.copy_from_slice(self.take(4)?);
        Ok(f32::from_le_bytes(value))
    }

    pub(super) fn finish(self) -> Result<(), ParagraphWireError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(ParagraphWireError::LengthMismatch)
        }
    }
}
