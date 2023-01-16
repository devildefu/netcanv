use std::borrow::Cow;
use std::sync::Mutex;

use image::RgbaImage;
use once_cell::sync::Lazy;

use crate::Error;

/// Initializes the clipboard in a platform-specific way.
#[allow(unused)]
pub fn init() -> netcanv::Result<()> {
   Ok(())
}

/// Copies the provided string into the clipboard.
pub fn copy_string(string: String) -> netcanv::Result<()> {
   todo!()
}

/// Copies the provided image into the clipboard.
pub fn copy_image(image: RgbaImage) -> netcanv::Result<()> {
   todo!()
}

/// Pastes the contents of the clipboard into a string.
pub fn paste_string() -> netcanv::Result<String> {
   todo!()
}

pub fn paste_image() -> netcanv::Result<RgbaImage> {
   todo!()
}
