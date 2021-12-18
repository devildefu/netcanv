#[cfg(not(any(target_arch = "wasm32")))]
mod socket;
#[cfg(target_arch = "wasm32")]
#[path = "socket_wasm.rs"]
mod socket;

pub use socket::*;
