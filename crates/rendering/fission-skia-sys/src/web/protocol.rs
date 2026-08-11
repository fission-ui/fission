//! Bounded encoder, decoder, and state gate for the CanvasKit bridge protocol.

use std::collections::BTreeMap;
use std::fmt;

use super::types::{
    Ack, AlphaMode, BackendPreference, ColorSpace, DamageRect, Destroy, DestroyReason, Envelope,
    ErrorCode, ErrorPacket, Frame, Init, Message, Packet, PacketKind, Resize, ResourceBatch,
    ResourceHandle, ResourceKind, ResourceOperation, ResourceUpdate, SessionId, SurfaceSize,
    HEADER_LEN, MAGIC, PROTOCOL_VERSION,
};

const INIT_PAYLOAD_LEN: usize = 16;
const RESIZE_PAYLOAD_LEN: usize = 12;
const RESOURCE_BATCH_PREFIX_LEN: usize = 16;
const RESOURCE_UPDATE_PREFIX_LEN: usize = 24;
const FRAME_PREFIX_LEN: usize = 64;
const DAMAGE_RECT_LEN: usize = 16;
const DESTROY_PAYLOAD_LEN: usize = 4;
const ACK_PAYLOAD_LEN: usize = 8;
const ERROR_PREFIX_LEN: usize = 16;

/// Allocation and numeric limits applied before decoding untrusted packets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecodeLimits {
    pub max_packet_bytes: usize,
    pub max_resource_updates: usize,
    pub max_resource_bytes: usize,
    pub max_resource_slots: usize,
    pub max_frame_commands: usize,
    pub max_damage_rects: usize,
    pub max_error_message_bytes: usize,
    pub max_surface_dimension: u32,
    pub max_scale_factor: f32,
}

pub const DEFAULT_DECODE_LIMITS: DecodeLimits = DecodeLimits {
    max_packet_bytes: 64 * 1024 * 1024,
    max_resource_updates: 4_096,
    max_resource_bytes: 32 * 1024 * 1024,
    max_resource_slots: 65_536,
    max_frame_commands: 32 * 1024 * 1024,
    max_damage_rects: 1_024,
    max_error_message_bytes: 4 * 1024,
    max_surface_dimension: 32_768,
    max_scale_factor: 16.0,
};

/// Structured rejection from binary or lifecycle validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Truncated {
        needed: usize,
        remaining: usize,
    },
    InvalidMagic,
    UnsupportedVersion(u16),
    UnknownPacketKind(u16),
    PacketTooLarge {
        actual: usize,
        maximum: usize,
    },
    LengthMismatch {
        declared: usize,
        actual: usize,
    },
    NonZeroFlags(u32),
    NonZeroReserved(&'static str),
    InvalidSession,
    InvalidSequence,
    KindMismatch {
        envelope: PacketKind,
        payload: PacketKind,
    },
    InvalidEnum {
        field: &'static str,
        value: u64,
    },
    InvalidValue(&'static str),
    LimitExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    TrailingBytes(usize),
    InvalidUtf8,
    SessionAlreadyActive(SessionId),
    NoActiveSession,
    StaleSession {
        latest: u64,
        received: u64,
    },
    UnexpectedSession {
        expected: SessionId,
        received: SessionId,
    },
    StaleSequence {
        latest: u64,
        received: u64,
    },
    UnexpectedSequence {
        expected: u64,
        received: u64,
    },
    StaleResourceEpoch {
        latest: u64,
        received: u64,
    },
    ResourceEpochMismatch {
        expected: u64,
        received: u64,
    },
    StaleFrame {
        latest: u64,
        received: u64,
    },
    StaleSemanticsEpoch {
        latest: u64,
        received: u64,
    },
    MissingResource(ResourceHandle),
    StaleResourceGeneration {
        slot: u32,
        latest: u32,
        received: u32,
    },
    ResourceSlotOccupied {
        slot: u32,
        generation: u32,
    },
    ReleasedResourceGeneration(ResourceHandle),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProtocolError {}

/// Encodes one complete packet using the canonical little-endian layout.
pub fn encode(message: &Message) -> Result<Vec<u8>, ProtocolError> {
    validate_message(message, &DEFAULT_DECODE_LIMITS)?;

    let payload_len = encoded_payload_len(&message.packet)?;
    let packet_len = HEADER_LEN
        .checked_add(payload_len)
        .ok_or(ProtocolError::PacketTooLarge {
            actual: usize::MAX,
            maximum: DEFAULT_DECODE_LIMITS.max_packet_bytes,
        })?;
    if packet_len > DEFAULT_DECODE_LIMITS.max_packet_bytes || packet_len > u32::MAX as usize {
        return Err(ProtocolError::PacketTooLarge {
            actual: packet_len,
            maximum: DEFAULT_DECODE_LIMITS
                .max_packet_bytes
                .min(u32::MAX as usize),
        });
    }

    let mut payload = Vec::with_capacity(payload_len);
    encode_payload(&message.packet, &mut payload)?;
    debug_assert_eq!(payload.len(), payload_len);
    let mut bytes = Vec::with_capacity(packet_len);
    bytes.extend_from_slice(&MAGIC);
    put_u16(&mut bytes, message.envelope.version);
    put_u16(&mut bytes, message.envelope.kind as u16);
    put_u32(&mut bytes, packet_len as u32);
    put_u32(&mut bytes, 0);
    put_u64(&mut bytes, message.envelope.session.get());
    put_u64(&mut bytes, message.envelope.sequence);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn encoded_payload_len(packet: &Packet) -> Result<usize, ProtocolError> {
    let checked_sum = |left: usize, right: usize, field: &'static str| {
        left.checked_add(right)
            .ok_or(ProtocolError::InvalidValue(field))
    };
    match packet {
        Packet::Init(_) => Ok(INIT_PAYLOAD_LEN),
        Packet::Resize(_) => Ok(RESIZE_PAYLOAD_LEN),
        Packet::ResourceBatch(batch) => {
            let mut len = RESOURCE_BATCH_PREFIX_LEN;
            for update in &batch.updates {
                len = checked_sum(
                    len,
                    RESOURCE_UPDATE_PREFIX_LEN,
                    "resource batch byte length",
                )?;
                len = checked_sum(len, update.bytes.len(), "resource batch byte length")?;
            }
            Ok(len)
        }
        Packet::Frame(frame) => {
            let damage_len = frame
                .damage
                .len()
                .checked_mul(DAMAGE_RECT_LEN)
                .ok_or(ProtocolError::InvalidValue("damage byte length"))?;
            let len = checked_sum(FRAME_PREFIX_LEN, damage_len, "frame payload byte length")?;
            checked_sum(len, frame.commands.len(), "frame payload byte length")
        }
        Packet::Destroy(_) => Ok(DESTROY_PAYLOAD_LEN),
        Packet::Ack(_) => Ok(ACK_PAYLOAD_LEN),
        Packet::Error(error) => checked_sum(
            ERROR_PREFIX_LEN,
            error.message.len(),
            "error payload byte length",
        ),
    }
}

/// Decodes and validates exactly one packet. Trailing bytes are rejected.
pub fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<Message, ProtocolError> {
    if bytes.len() > limits.max_packet_bytes {
        return Err(ProtocolError::PacketTooLarge {
            actual: bytes.len(),
            maximum: limits.max_packet_bytes,
        });
    }
    if bytes.len() < HEADER_LEN {
        return Err(ProtocolError::Truncated {
            needed: HEADER_LEN,
            remaining: bytes.len(),
        });
    }

    let mut cursor = Cursor::new(bytes);
    if cursor.take(4)? != MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = cursor.u16()?;
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }
    let kind_value = cursor.u16()?;
    let kind =
        PacketKind::from_wire(kind_value).ok_or(ProtocolError::UnknownPacketKind(kind_value))?;
    let declared_len = cursor.u32()? as usize;
    if declared_len != bytes.len() {
        return Err(ProtocolError::LengthMismatch {
            declared: declared_len,
            actual: bytes.len(),
        });
    }
    let flags = cursor.u32()?;
    if flags != 0 {
        return Err(ProtocolError::NonZeroFlags(flags));
    }
    let session_value = cursor.u64()?;
    let session = SessionId::new(session_value).ok_or(ProtocolError::InvalidSession)?;
    let sequence = cursor.u64()?;
    if sequence == 0 {
        return Err(ProtocolError::InvalidSequence);
    }

    let packet = decode_payload(kind, &mut cursor, limits)?;
    cursor.finish()?;
    let message = Message {
        envelope: Envelope {
            version,
            kind,
            session,
            sequence,
        },
        packet,
    };
    validate_message(&message, limits)?;
    Ok(message)
}

fn encode_payload(packet: &Packet, bytes: &mut Vec<u8>) -> Result<(), ProtocolError> {
    match packet {
        Packet::Init(init) => {
            encode_surface(init.surface, bytes);
            bytes.push(init.backend as u8);
            bytes.push(init.color_space as u8);
            bytes.push(init.alpha_mode as u8);
            bytes.push(0);
        }
        Packet::Resize(resize) => encode_surface(resize.surface, bytes),
        Packet::ResourceBatch(batch) => {
            put_u64(bytes, batch.resource_epoch);
            put_u32(
                bytes,
                usize_to_u32(batch.updates.len(), "resource updates")?,
            );
            put_u32(bytes, 0);
            for update in &batch.updates {
                put_u32(bytes, update.handle.slot);
                put_u32(bytes, update.handle.generation);
                bytes.push(update.operation as u8);
                bytes.push(update.kind as u8);
                put_u16(bytes, 0);
                put_u64(bytes, update.content_id);
                put_u32(bytes, usize_to_u32(update.bytes.len(), "resource bytes")?);
                bytes.extend_from_slice(&update.bytes);
            }
        }
        Packet::Frame(frame) => {
            put_u64(bytes, frame.frame_id);
            put_u64(bytes, frame.resource_epoch);
            put_u64(bytes, frame.semantics_epoch);
            encode_surface(frame.surface, bytes);
            put_u32(
                bytes,
                usize_to_u32(frame.damage.len(), "damage rectangles")?,
            );
            put_u32(bytes, usize_to_u32(frame.commands.len(), "frame commands")?);
            for component in frame.clear_color {
                put_f32(bytes, component);
            }
            put_u32(bytes, 0);
            for damage in &frame.damage {
                put_u32(bytes, damage.x);
                put_u32(bytes, damage.y);
                put_u32(bytes, damage.width);
                put_u32(bytes, damage.height);
            }
            bytes.extend_from_slice(&frame.commands);
        }
        Packet::Destroy(destroy) => {
            put_u16(bytes, destroy.reason as u16);
            put_u16(bytes, 0);
        }
        Packet::Ack(ack) => put_u64(bytes, ack.acknowledged_sequence),
        Packet::Error(error) => {
            put_u64(bytes, error.failed_sequence);
            put_u16(bytes, error.code as u16);
            put_u16(bytes, 0);
            put_u32(bytes, usize_to_u32(error.message.len(), "error message")?);
            bytes.extend_from_slice(error.message.as_bytes());
        }
    }
    Ok(())
}

fn decode_payload(
    kind: PacketKind,
    cursor: &mut Cursor<'_>,
    limits: &DecodeLimits,
) -> Result<Packet, ProtocolError> {
    match kind {
        PacketKind::Init => {
            require_remaining(cursor, INIT_PAYLOAD_LEN)?;
            let surface = decode_surface(cursor)?;
            let backend_value = cursor.u8()?;
            let backend =
                BackendPreference::from_wire(backend_value).ok_or(ProtocolError::InvalidEnum {
                    field: "backend preference",
                    value: backend_value as u64,
                })?;
            let color_value = cursor.u8()?;
            let color_space =
                ColorSpace::from_wire(color_value).ok_or(ProtocolError::InvalidEnum {
                    field: "color space",
                    value: color_value as u64,
                })?;
            let alpha_value = cursor.u8()?;
            let alpha_mode =
                AlphaMode::from_wire(alpha_value).ok_or(ProtocolError::InvalidEnum {
                    field: "alpha mode",
                    value: alpha_value as u64,
                })?;
            require_zero(cursor.u8()? as u64, "init")?;
            Ok(Packet::Init(Init {
                surface,
                backend,
                color_space,
                alpha_mode,
            }))
        }
        PacketKind::Resize => {
            require_remaining(cursor, RESIZE_PAYLOAD_LEN)?;
            Ok(Packet::Resize(Resize {
                surface: decode_surface(cursor)?,
            }))
        }
        PacketKind::ResourceBatch => decode_resource_batch(cursor, limits),
        PacketKind::Frame => decode_frame(cursor, limits),
        PacketKind::Destroy => {
            require_remaining(cursor, DESTROY_PAYLOAD_LEN)?;
            let reason_value = cursor.u16()?;
            let reason =
                DestroyReason::from_wire(reason_value).ok_or(ProtocolError::InvalidEnum {
                    field: "destroy reason",
                    value: reason_value as u64,
                })?;
            require_zero(cursor.u16()? as u64, "destroy")?;
            Ok(Packet::Destroy(Destroy { reason }))
        }
        PacketKind::Ack => {
            require_remaining(cursor, ACK_PAYLOAD_LEN)?;
            Ok(Packet::Ack(Ack {
                acknowledged_sequence: cursor.u64()?,
            }))
        }
        PacketKind::Error => decode_error(cursor, limits),
    }
}

fn decode_resource_batch(
    cursor: &mut Cursor<'_>,
    limits: &DecodeLimits,
) -> Result<Packet, ProtocolError> {
    require_at_least(cursor, RESOURCE_BATCH_PREFIX_LEN)?;
    let resource_epoch = cursor.u64()?;
    let update_count = cursor.u32()? as usize;
    require_limit(
        "resource updates",
        update_count,
        limits.max_resource_updates,
    )?;
    require_zero(cursor.u32()? as u64, "resource batch")?;

    let minimum_prefix_bytes = update_count
        .checked_mul(RESOURCE_UPDATE_PREFIX_LEN)
        .ok_or(ProtocolError::InvalidValue("resource update byte length"))?;
    require_at_least(cursor, minimum_prefix_bytes)?;

    let mut updates = Vec::with_capacity(update_count);
    for _ in 0..update_count {
        let handle = ResourceHandle {
            slot: cursor.u32()?,
            generation: cursor.u32()?,
        };
        let operation_value = cursor.u8()?;
        let operation =
            ResourceOperation::from_wire(operation_value).ok_or(ProtocolError::InvalidEnum {
                field: "resource operation",
                value: operation_value as u64,
            })?;
        let kind_value = cursor.u8()?;
        let kind = ResourceKind::from_wire(kind_value).ok_or(ProtocolError::InvalidEnum {
            field: "resource kind",
            value: kind_value as u64,
        })?;
        require_zero(cursor.u16()? as u64, "resource update")?;
        let content_id = cursor.u64()?;
        let byte_len = cursor.u32()? as usize;
        require_limit("resource bytes", byte_len, limits.max_resource_bytes)?;
        let bytes = cursor.take(byte_len)?.to_vec();
        updates.push(ResourceUpdate {
            handle,
            operation,
            kind,
            content_id,
            bytes,
        });
    }
    Ok(Packet::ResourceBatch(ResourceBatch {
        resource_epoch,
        updates,
    }))
}

fn decode_frame(cursor: &mut Cursor<'_>, limits: &DecodeLimits) -> Result<Packet, ProtocolError> {
    require_at_least(cursor, FRAME_PREFIX_LEN)?;
    let frame_id = cursor.u64()?;
    let resource_epoch = cursor.u64()?;
    let semantics_epoch = cursor.u64()?;
    let surface = decode_surface(cursor)?;
    let damage_count = cursor.u32()? as usize;
    require_limit("damage rectangles", damage_count, limits.max_damage_rects)?;
    let command_len = cursor.u32()? as usize;
    require_limit("frame commands", command_len, limits.max_frame_commands)?;
    let mut clear_color = [0.0; 4];
    for component in &mut clear_color {
        *component = cursor.f32()?;
    }
    require_zero(cursor.u32()? as u64, "frame")?;

    let damage_bytes = damage_count
        .checked_mul(DAMAGE_RECT_LEN)
        .ok_or(ProtocolError::InvalidValue("damage byte length"))?;
    let remaining = damage_bytes
        .checked_add(command_len)
        .ok_or(ProtocolError::InvalidValue("frame payload byte length"))?;
    require_remaining(cursor, remaining)?;

    let mut damage = Vec::with_capacity(damage_count);
    for _ in 0..damage_count {
        damage.push(DamageRect {
            x: cursor.u32()?,
            y: cursor.u32()?,
            width: cursor.u32()?,
            height: cursor.u32()?,
        });
    }
    let commands = cursor.take(command_len)?.to_vec();
    Ok(Packet::Frame(Frame {
        frame_id,
        resource_epoch,
        semantics_epoch,
        surface,
        clear_color,
        damage,
        commands,
    }))
}

fn decode_error(cursor: &mut Cursor<'_>, limits: &DecodeLimits) -> Result<Packet, ProtocolError> {
    require_at_least(cursor, ERROR_PREFIX_LEN)?;
    let failed_sequence = cursor.u64()?;
    let code_value = cursor.u16()?;
    let code = ErrorCode::from_wire(code_value).ok_or(ProtocolError::InvalidEnum {
        field: "error code",
        value: code_value as u64,
    })?;
    require_zero(cursor.u16()? as u64, "error")?;
    let message_len = cursor.u32()? as usize;
    require_limit("error message", message_len, limits.max_error_message_bytes)?;
    let message = std::str::from_utf8(cursor.take(message_len)?)
        .map_err(|_| ProtocolError::InvalidUtf8)?
        .to_owned();
    Ok(Packet::Error(ErrorPacket {
        failed_sequence,
        code,
        message,
    }))
}

fn validate_message(message: &Message, limits: &DecodeLimits) -> Result<(), ProtocolError> {
    if message.envelope.version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(message.envelope.version));
    }
    if message.envelope.kind != message.packet.kind() {
        return Err(ProtocolError::KindMismatch {
            envelope: message.envelope.kind,
            payload: message.packet.kind(),
        });
    }
    if message.envelope.sequence == 0 {
        return Err(ProtocolError::InvalidSequence);
    }

    match &message.packet {
        Packet::Init(init) => validate_surface(init.surface, limits),
        Packet::Resize(resize) => validate_surface(resize.surface, limits),
        Packet::ResourceBatch(batch) => validate_resource_batch(batch, limits),
        Packet::Frame(frame) => validate_frame(frame, limits),
        Packet::Destroy(_) => Ok(()),
        Packet::Ack(ack) => {
            if ack.acknowledged_sequence == 0 {
                Err(ProtocolError::InvalidValue("acknowledged sequence"))
            } else {
                Ok(())
            }
        }
        Packet::Error(error) => {
            if error.failed_sequence == 0 {
                return Err(ProtocolError::InvalidValue("failed sequence"));
            }
            require_limit(
                "error message",
                error.message.len(),
                limits.max_error_message_bytes,
            )
        }
    }
}

fn validate_resource_batch(
    batch: &ResourceBatch,
    limits: &DecodeLimits,
) -> Result<(), ProtocolError> {
    if batch.resource_epoch == 0 {
        return Err(ProtocolError::InvalidValue("resource epoch"));
    }
    require_limit(
        "resource updates",
        batch.updates.len(),
        limits.max_resource_updates,
    )?;
    for update in &batch.updates {
        if update.handle.slot == 0 {
            return Err(ProtocolError::InvalidValue("resource slot"));
        }
        if update.handle.generation == 0 {
            return Err(ProtocolError::InvalidValue("resource generation"));
        }
        require_limit(
            "resource bytes",
            update.bytes.len(),
            limits.max_resource_bytes,
        )?;
        match update.operation {
            ResourceOperation::Upsert if update.content_id == 0 => {
                return Err(ProtocolError::InvalidValue("resource content id"));
            }
            ResourceOperation::Release if update.content_id != 0 || !update.bytes.is_empty() => {
                return Err(ProtocolError::InvalidValue("released resource payload"));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_frame(frame: &Frame, limits: &DecodeLimits) -> Result<(), ProtocolError> {
    if frame.frame_id == 0 {
        return Err(ProtocolError::InvalidValue("frame id"));
    }
    validate_surface(frame.surface, limits)?;
    require_limit(
        "damage rectangles",
        frame.damage.len(),
        limits.max_damage_rects,
    )?;
    require_limit(
        "frame commands",
        frame.commands.len(),
        limits.max_frame_commands,
    )?;
    if frame
        .clear_color
        .iter()
        .any(|component| !component.is_finite() || !(0.0..=1.0).contains(component))
    {
        return Err(ProtocolError::InvalidValue("clear color"));
    }
    for damage in &frame.damage {
        let right = damage
            .x
            .checked_add(damage.width)
            .ok_or(ProtocolError::InvalidValue("damage rectangle"))?;
        let bottom = damage
            .y
            .checked_add(damage.height)
            .ok_or(ProtocolError::InvalidValue("damage rectangle"))?;
        if right > frame.surface.width || bottom > frame.surface.height {
            return Err(ProtocolError::InvalidValue("damage rectangle bounds"));
        }
    }
    Ok(())
}

fn validate_surface(surface: SurfaceSize, limits: &DecodeLimits) -> Result<(), ProtocolError> {
    if surface.width > limits.max_surface_dimension || surface.height > limits.max_surface_dimension
    {
        return Err(ProtocolError::LimitExceeded {
            field: "surface dimension",
            actual: surface.width.max(surface.height) as usize,
            maximum: limits.max_surface_dimension as usize,
        });
    }
    if !surface.scale_factor.is_finite()
        || surface.scale_factor <= 0.0
        || surface.scale_factor > limits.max_scale_factor
    {
        return Err(ProtocolError::InvalidValue("surface scale factor"));
    }
    Ok(())
}

fn encode_surface(surface: SurfaceSize, bytes: &mut Vec<u8>) {
    put_u32(bytes, surface.width);
    put_u32(bytes, surface.height);
    put_f32(bytes, surface.scale_factor);
}

fn decode_surface(cursor: &mut Cursor<'_>) -> Result<SurfaceSize, ProtocolError> {
    Ok(SurfaceSize {
        width: cursor.u32()?,
        height: cursor.u32()?,
        scale_factor: cursor.f32()?,
    })
}

fn usize_to_u32(value: usize, field: &'static str) -> Result<u32, ProtocolError> {
    u32::try_from(value).map_err(|_| ProtocolError::LimitExceeded {
        field,
        actual: value,
        maximum: u32::MAX as usize,
    })
}

fn require_limit(field: &'static str, actual: usize, maximum: usize) -> Result<(), ProtocolError> {
    if actual > maximum {
        Err(ProtocolError::LimitExceeded {
            field,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn require_zero(value: u64, field: &'static str) -> Result<(), ProtocolError> {
    if value == 0 {
        Ok(())
    } else {
        Err(ProtocolError::NonZeroReserved(field))
    }
}

fn require_at_least(cursor: &Cursor<'_>, needed: usize) -> Result<(), ProtocolError> {
    if cursor.remaining() < needed {
        Err(ProtocolError::Truncated {
            needed,
            remaining: cursor.remaining(),
        })
    } else {
        Ok(())
    }
}

fn require_remaining(cursor: &Cursor<'_>, needed: usize) -> Result<(), ProtocolError> {
    if cursor.remaining() != needed {
        Err(ProtocolError::LengthMismatch {
            declared: needed,
            actual: cursor.remaining(),
        })
    } else {
        Ok(())
    }
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
        if self.remaining() < len {
            return Err(ProtocolError::Truncated {
                needed: len,
                remaining: self.remaining(),
            });
        }
        let end = self
            .position
            .checked_add(len)
            .ok_or(ProtocolError::InvalidValue("cursor position"))?;
        let value = &self.bytes[self.position..end];
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        let mut bytes = [0; 2];
        bytes.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn f32(&mut self) -> Result<f32, ProtocolError> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(f32::from_le_bytes(bytes))
    }

    fn finish(&self) -> Result<(), ProtocolError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(ProtocolError::TrailingBytes(self.remaining()))
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResourceSlotState {
    generation: u32,
    live: bool,
}

#[derive(Debug, Clone)]
struct ActiveSession {
    id: SessionId,
    last_sequence: u64,
    resource_epoch: u64,
    last_frame_id: u64,
    semantics_epoch: u64,
    resources: BTreeMap<u32, ResourceSlotState>,
}

/// Stateful command-stream gate used before mutating CanvasKit-owned objects.
///
/// Sessions and sequences are contiguous and monotonically increasing. Resource
/// batches are applied atomically, and a released slot can only be reused with a
/// higher generation.
#[derive(Debug, Clone)]
pub struct ProtocolSession {
    limits: DecodeLimits,
    latest_session: u64,
    active: Option<ActiveSession>,
}

impl Default for ProtocolSession {
    fn default() -> Self {
        Self::new(DEFAULT_DECODE_LIMITS)
    }
}

impl ProtocolSession {
    pub const fn new(limits: DecodeLimits) -> Self {
        Self {
            limits,
            latest_session: 0,
            active: None,
        }
    }

    pub fn active_session(&self) -> Option<SessionId> {
        self.active.as_ref().map(|active| active.id)
    }

    pub fn live_resource_count(&self) -> usize {
        self.active
            .as_ref()
            .map(|active| active.resources.values().filter(|state| state.live).count())
            .unwrap_or(0)
    }

    /// Atomically validates and advances the command stream.
    pub fn accept(&mut self, message: &Message) -> Result<(), ProtocolError> {
        validate_message(message, &self.limits)?;

        if let Packet::Init(_) = &message.packet {
            return self.accept_init(message);
        }

        let active = self.active.as_ref().ok_or(ProtocolError::NoActiveSession)?;
        if message.envelope.session != active.id {
            if message.envelope.session.get() <= self.latest_session {
                return Err(ProtocolError::StaleSession {
                    latest: self.latest_session,
                    received: message.envelope.session.get(),
                });
            }
            return Err(ProtocolError::UnexpectedSession {
                expected: active.id,
                received: message.envelope.session,
            });
        }
        let expected_sequence = active
            .last_sequence
            .checked_add(1)
            .ok_or(ProtocolError::InvalidSequence)?;
        if message.envelope.sequence < expected_sequence {
            return Err(ProtocolError::StaleSequence {
                latest: active.last_sequence,
                received: message.envelope.sequence,
            });
        }
        if message.envelope.sequence > expected_sequence {
            return Err(ProtocolError::UnexpectedSequence {
                expected: expected_sequence,
                received: message.envelope.sequence,
            });
        }

        let mut next = active.clone();
        match &message.packet {
            Packet::Init(_) => unreachable!("init handled before active-session validation"),
            Packet::Resize(_) | Packet::Ack(_) | Packet::Error(_) => {}
            Packet::ResourceBatch(batch) => self.apply_resource_batch(&mut next, batch)?,
            Packet::Frame(frame) => apply_frame(&mut next, frame)?,
            Packet::Destroy(_) => {
                self.active = None;
                return Ok(());
            }
        }
        next.last_sequence = message.envelope.sequence;
        self.active = Some(next);
        Ok(())
    }

    fn accept_init(&mut self, message: &Message) -> Result<(), ProtocolError> {
        if let Some(active) = &self.active {
            return Err(ProtocolError::SessionAlreadyActive(active.id));
        }
        let session = message.envelope.session;
        if session.get() <= self.latest_session {
            return Err(ProtocolError::StaleSession {
                latest: self.latest_session,
                received: session.get(),
            });
        }
        if message.envelope.sequence != 1 {
            return Err(ProtocolError::UnexpectedSequence {
                expected: 1,
                received: message.envelope.sequence,
            });
        }

        self.latest_session = session.get();
        self.active = Some(ActiveSession {
            id: session,
            last_sequence: 1,
            resource_epoch: 0,
            last_frame_id: 0,
            semantics_epoch: 0,
            resources: BTreeMap::new(),
        });
        Ok(())
    }

    fn apply_resource_batch(
        &self,
        active: &mut ActiveSession,
        batch: &ResourceBatch,
    ) -> Result<(), ProtocolError> {
        if batch.resource_epoch <= active.resource_epoch {
            return Err(ProtocolError::StaleResourceEpoch {
                latest: active.resource_epoch,
                received: batch.resource_epoch,
            });
        }

        let mut resources = active.resources.clone();
        for update in &batch.updates {
            apply_resource_update(&mut resources, update)?;
            require_limit(
                "resource slots",
                resources.len(),
                self.limits.max_resource_slots,
            )?;
        }
        active.resources = resources;
        active.resource_epoch = batch.resource_epoch;
        Ok(())
    }
}

fn apply_resource_update(
    resources: &mut BTreeMap<u32, ResourceSlotState>,
    update: &ResourceUpdate,
) -> Result<(), ProtocolError> {
    let previous = resources.get(&update.handle.slot).copied();
    match (update.operation, previous) {
        (ResourceOperation::Upsert, None) => {
            resources.insert(
                update.handle.slot,
                ResourceSlotState {
                    generation: update.handle.generation,
                    live: true,
                },
            );
        }
        (ResourceOperation::Upsert, Some(state)) if update.handle.generation < state.generation => {
            return Err(ProtocolError::StaleResourceGeneration {
                slot: update.handle.slot,
                latest: state.generation,
                received: update.handle.generation,
            });
        }
        (ResourceOperation::Upsert, Some(state))
            if update.handle.generation == state.generation && !state.live =>
        {
            return Err(ProtocolError::ReleasedResourceGeneration(update.handle));
        }
        (ResourceOperation::Upsert, Some(state))
            if update.handle.generation > state.generation && state.live =>
        {
            return Err(ProtocolError::ResourceSlotOccupied {
                slot: update.handle.slot,
                generation: state.generation,
            });
        }
        (ResourceOperation::Upsert, Some(_)) => {
            resources.insert(
                update.handle.slot,
                ResourceSlotState {
                    generation: update.handle.generation,
                    live: true,
                },
            );
        }
        (ResourceOperation::Release, None) => {
            return Err(ProtocolError::MissingResource(update.handle));
        }
        (ResourceOperation::Release, Some(state))
            if update.handle.generation < state.generation =>
        {
            return Err(ProtocolError::StaleResourceGeneration {
                slot: update.handle.slot,
                latest: state.generation,
                received: update.handle.generation,
            });
        }
        (ResourceOperation::Release, Some(state))
            if update.handle.generation > state.generation =>
        {
            return Err(ProtocolError::MissingResource(update.handle));
        }
        (ResourceOperation::Release, Some(state)) if !state.live => {
            return Err(ProtocolError::ReleasedResourceGeneration(update.handle));
        }
        (ResourceOperation::Release, Some(mut state)) => {
            state.live = false;
            resources.insert(update.handle.slot, state);
        }
    }
    Ok(())
}

fn apply_frame(active: &mut ActiveSession, frame: &Frame) -> Result<(), ProtocolError> {
    if frame.frame_id <= active.last_frame_id {
        return Err(ProtocolError::StaleFrame {
            latest: active.last_frame_id,
            received: frame.frame_id,
        });
    }
    if frame.resource_epoch != active.resource_epoch {
        return Err(ProtocolError::ResourceEpochMismatch {
            expected: active.resource_epoch,
            received: frame.resource_epoch,
        });
    }
    if frame.semantics_epoch < active.semantics_epoch {
        return Err(ProtocolError::StaleSemanticsEpoch {
            latest: active.semantics_epoch,
            received: frame.semantics_epoch,
        });
    }
    active.last_frame_id = frame.frame_id;
    active.semantics_epoch = frame.semantics_epoch;
    Ok(())
}
