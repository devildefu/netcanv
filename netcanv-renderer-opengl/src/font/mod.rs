#[cfg(not(target_arch = "wasm32"))]
#[path = "freetype.rs"]
mod font;

#[cfg(target_arch = "wasm32")]
#[path = "ab_glyph.rs"]
mod font;

pub use font::*;
