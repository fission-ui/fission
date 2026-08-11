#![doc = include_str!("../README.md")]

mod capabilities;
mod fonts;
mod renderer;
mod resources;
mod shadows;

pub use capabilities::software_backend_capabilities;
pub use fonts::register_packaged_fonts;
pub use renderer::SoftwareRenderer;
pub use resources::{
    image_cache_generation, image_cache_has_pending, image_cache_stats, ImageCacheStats,
};
