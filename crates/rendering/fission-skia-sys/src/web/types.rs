//! Values carried by the CanvasKit bridge protocol.

/// Four-byte prefix on every CanvasKit bridge packet (`FSKN`).
pub const MAGIC: [u8; 4] = *b"FSKN";

/// Current CanvasKit bridge protocol version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Byte length of a version-one packet envelope.
pub const HEADER_LEN: usize = 32;

/// Identifies the payload stored after an [`Envelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum PacketKind {
    Init = 1,
    Resize = 2,
    ResourceBatch = 3,
    Frame = 4,
    Destroy = 5,
    Ack = 6,
    Error = 7,
}

impl PacketKind {
    pub(crate) fn from_wire(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Init),
            2 => Some(Self::Resize),
            3 => Some(Self::ResourceBatch),
            4 => Some(Self::Frame),
            5 => Some(Self::Destroy),
            6 => Some(Self::Ack),
            7 => Some(Self::Error),
            _ => None,
        }
    }
}

/// Non-zero, monotonically increasing identity for one bridge lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(u64);

impl SessionId {
    /// Creates a session identifier. Zero is reserved for malformed input.
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }

    /// Returns the wire value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Common header for every protocol packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envelope {
    pub version: u16,
    pub kind: PacketKind,
    pub session: SessionId,
    pub sequence: u64,
}

/// Browser rendering preference requested at session creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BackendPreference {
    Auto = 0,
    WebGl = 1,
    Graphite = 2,
    Software = 3,
}

impl BackendPreference {
    pub(crate) fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Auto),
            1 => Some(Self::WebGl),
            2 => Some(Self::Graphite),
            3 => Some(Self::Software),
            _ => None,
        }
    }
}

/// Destination color space requested from CanvasKit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorSpace {
    Srgb = 1,
    DisplayP3 = 2,
}

impl ColorSpace {
    pub(crate) fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Srgb),
            2 => Some(Self::DisplayP3),
            _ => None,
        }
    }
}

/// Canvas alpha representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AlphaMode {
    Opaque = 1,
    Premultiplied = 2,
}

impl AlphaMode {
    pub(crate) fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Opaque),
            2 => Some(Self::Premultiplied),
            _ => None,
        }
    }
}

/// Physical browser surface dimensions and its device-pixel ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

/// Establishes a new CanvasKit bridge session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Init {
    pub surface: SurfaceSize,
    pub backend: BackendPreference,
    pub color_space: ColorSpace,
    pub alpha_mode: AlphaMode,
}

/// Updates the physical surface dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resize {
    pub surface: SurfaceSize,
}

/// Generational resource identity. Slot zero and generation zero are invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceHandle {
    pub slot: u32,
    pub generation: u32,
}

/// Resource payload interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResourceKind {
    Image = 1,
    Svg = 2,
    Font = 3,
    Text = 4,
    Binary = 5,
}

impl ResourceKind {
    pub(crate) fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Image),
            2 => Some(Self::Svg),
            3 => Some(Self::Font),
            4 => Some(Self::Text),
            5 => Some(Self::Binary),
            _ => None,
        }
    }
}

/// Mutation applied to one resource-table slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResourceOperation {
    Upsert = 1,
    Release = 2,
}

impl ResourceOperation {
    pub(crate) fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Upsert),
            2 => Some(Self::Release),
            _ => None,
        }
    }
}

/// One atomic resource-table update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceUpdate {
    pub handle: ResourceHandle,
    pub operation: ResourceOperation,
    pub kind: ResourceKind,
    /// Stable content identity. It is non-zero for upserts and zero for releases.
    pub content_id: u64,
    pub bytes: Vec<u8>,
}

/// Resource mutations associated with one retained resource epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBatch {
    pub resource_epoch: u64,
    pub updates: Vec<ResourceUpdate>,
}

/// Integer damage rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One immutable, batched CanvasKit frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub frame_id: u64,
    pub resource_epoch: u64,
    pub semantics_epoch: u64,
    pub surface: SurfaceSize,
    pub clear_color: [f32; 4],
    pub damage: Vec<DamageRect>,
    /// Backend-private packed commands. Version one treats these as opaque bytes.
    pub commands: Vec<u8>,
}

/// Why the host is closing the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DestroyReason {
    Normal = 0,
    Replaced = 1,
    HostShutdown = 2,
    ContextLost = 3,
}

impl DestroyReason {
    pub(crate) fn from_wire(value: u16) -> Option<Self> {
        match value {
            0 => Some(Self::Normal),
            1 => Some(Self::Replaced),
            2 => Some(Self::HostShutdown),
            3 => Some(Self::ContextLost),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destroy {
    pub reason: DestroyReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ack {
    pub acknowledged_sequence: u64,
}

/// Stable error category crossing the Wasm-module boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorCode {
    InvalidPacket = 1,
    UnsupportedVersion = 2,
    InvalidState = 3,
    ResourceFailure = 4,
    SurfaceLost = 5,
    Internal = 6,
}

impl ErrorCode {
    pub(crate) fn from_wire(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::InvalidPacket),
            2 => Some(Self::UnsupportedVersion),
            3 => Some(Self::InvalidState),
            4 => Some(Self::ResourceFailure),
            5 => Some(Self::SurfaceLost),
            6 => Some(Self::Internal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorPacket {
    pub failed_sequence: u64,
    pub code: ErrorCode,
    pub message: String,
}

/// Typed protocol payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Packet {
    Init(Init),
    Resize(Resize),
    ResourceBatch(ResourceBatch),
    Frame(Frame),
    Destroy(Destroy),
    Ack(Ack),
    Error(ErrorPacket),
}

impl Packet {
    pub const fn kind(&self) -> PacketKind {
        match self {
            Self::Init(_) => PacketKind::Init,
            Self::Resize(_) => PacketKind::Resize,
            Self::ResourceBatch(_) => PacketKind::ResourceBatch,
            Self::Frame(_) => PacketKind::Frame,
            Self::Destroy(_) => PacketKind::Destroy,
            Self::Ack(_) => PacketKind::Ack,
            Self::Error(_) => PacketKind::Error,
        }
    }
}

/// Envelope and decoded payload transferred as one unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub envelope: Envelope,
    pub packet: Packet,
}

impl Message {
    pub fn new(session: SessionId, sequence: u64, packet: Packet) -> Self {
        Self {
            envelope: Envelope {
                version: PROTOCOL_VERSION,
                kind: packet.kind(),
                session,
                sequence,
            },
            packet,
        }
    }
}
