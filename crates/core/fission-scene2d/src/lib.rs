//! Closed, retained 2D scenes for Fission.
//!
//! [`Scene2DIR`] is the serialisable authority supplied by an application.
//! [`Scene2DProcessor`] validates it and produces a deterministic
//! [`PreparedScene2D`] for renderers, picking, and headless tests. Repeated image
//! and sprite instances remain one draw command throughout this crate; a
//! renderer adapter must preserve that command as a batch.

mod geometry;
mod interaction;
mod ir;
mod processing;
mod widget;

pub use geometry::{Affine2, Rect2D};
pub use interaction::{
    ActionBinding2D, ActionToken, DragActions2D, Interaction2D, InteractionEvent2D, SemanticRole2D,
};
pub use ir::{
    BlendMode2D, Camera2D, Clip2D, Fill2D, Glyph2D, ImageBatch2D, ImageHandle2D, ImageInstance2D,
    ImageSampling2D, ImageSource2D, Node2D, NodeContent2D, PathCommand2D, PathResource2D,
    PathStyle2D, ResourceId, Scene2DIR, Scene2DResources, Sprite2D, SpriteBatch2D, SpriteFrame2D,
    SpriteInstance2D, SpriteSheet2D, Stroke2D, TextResource2D, Viewport2D,
};
pub use processing::{
    DrawCommand2D, DrawMetadata2D, PickHit2D, PreparedClip2D, PreparedImageInstance2D,
    PreparedScene2D, PreparedSpriteInstance2D, Scene2DProcessor,
};
pub use widget::{Scene2D, Scene2DRenderPacket, SCENE2D_EMBED_MAGIC};

/// Wire-format version of the first public 2D scene alpha.
pub const SCENE2D_FORMAT_VERSION: u32 = 1;
