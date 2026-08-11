#![forbid(unsafe_code)]

//! Optional wgpu implementation of Fission's neutral 3D scene model.

pub mod render;

pub use fission_3d_model::{Scene3DSource, Scene3DValidationError};
pub use render::{Scene3DComposition, Scene3DRenderer, Scene3DViewport, Vertex};
