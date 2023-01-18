use gloo_storage::{LocalStorage, Storage};
use image::codecs::png::PngEncoder;
use image::{load_from_memory_with_format, ColorType, ImageFormat, RgbaImage, ImageEncoder};
use js_sys::Uint8Array;
use once_cell::sync::Lazy;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};

use wasm_bindgen::prelude::*;

use crate::image_coder::ImageCoder;

static CLIPBOARD_READ: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static CLIPBOARD_WRITE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

#[wasm_bindgen(raw_module = "clipboard")]
extern "C" {
   #[wasm_bindgen(js_name = "askForPermission", catch)]
   async fn _ask_for_permission(name: &str) -> Result<JsValue, JsValue>;

   #[wasm_bindgen(js_name = "init")]
   fn _init();

   #[wasm_bindgen(js_name = "copyString")]
   fn _copy_string(string: &str);

   #[wasm_bindgen(js_name = "copyImage")]
   fn _copy_image(image: &[u8]);

   #[wasm_bindgen(js_name = "pasteString")]
   fn _paste_string() -> String;

   #[wasm_bindgen(js_name = "pasteImage", catch)]
   async fn _paste_image() -> Result<JsValue, JsValue>;
}

async fn ask_for_permission(name: &str) -> Result<bool, JsValue> {
   let result = _ask_for_permission(name).await?;
   Ok(result.as_bool().unwrap())
}

pub fn init() -> netcanv::Result<()> {
   // Clipboard MAY work on firefox/safari, so I'll hide option to force it
   // in localStorage for now.
   match LocalStorage::get("_FORCE_CLIPBOARD") {
      Ok(v) if v => {
         log::info!("Forced clipboard");

         CLIPBOARD_READ.store(true, Ordering::Relaxed);
         CLIPBOARD_WRITE.store(true, Ordering::Relaxed);

         return Ok(());
      }
      _ => (),
   }

   wasm_bindgen_futures::spawn_local(async {
      let read = ask_for_permission("clipboard-read").await.unwrap();
      CLIPBOARD_READ.store(read, Ordering::Relaxed);

      let write = ask_for_permission("clipboard-write").await.unwrap();
      CLIPBOARD_WRITE.store(write, Ordering::Relaxed);
   });

   _init();

   Ok(())
}

pub fn copy_string(string: String) -> netcanv::Result<()> {
   if CLIPBOARD_WRITE.load(Ordering::Relaxed) {
      _copy_string(&string);
      Ok(())
   } else {
      Err(netcanv::Error::ClipboardUnknown {
         error: "no permissions to copy text to clipboard".to_string(),
      })
   }
}

pub fn copy_image(image: RgbaImage) -> netcanv::Result<()> {
   if CLIPBOARD_WRITE.load(Ordering::Relaxed) {
      let buf = ImageCoder::encode_png_data(image)?;
      _copy_image(buf.as_slice());

      Ok(())
   } else {
      Err(netcanv::Error::ClipboardUnknown {
         error: "no permissions to copy image to clipboard".to_string(),
      })
   }
}

pub fn paste_string() -> netcanv::Result<String> {
   if CLIPBOARD_READ.load(Ordering::Relaxed) {
      let string = _paste_string();
      log::debug!("{}", string);
      Ok(string)
   } else {
      Err(netcanv::Error::ClipboardUnknown {
         error: "no permissions to paste image from clipboard".to_string(),
      })
   }
}

pub async fn paste_image() -> netcanv::Result<RgbaImage> {
   if CLIPBOARD_READ.load(Ordering::Relaxed) {
      let buffer = _paste_image().await.unwrap();
      let bytes = Uint8Array::new(&buffer).to_vec();
      Ok(load_from_memory_with_format(&bytes, ImageFormat::Png)?.to_rgba8())
   } else {
      Err(netcanv::Error::ClipboardUnknown {
         error: "no permissions to paste image from clipboard".to_string(),
      })
   }
}
