#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard;
#[cfg(target_arch = "wasm32")]
#[path = "clipboard_wasm.rs"]
pub mod clipboard;

pub use clipboard::*;
