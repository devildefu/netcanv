use gloo_storage::{LocalStorage, Storage};
use image::png::PngEncoder;
use image::{ColorType, RgbaImage};
use once_cell::sync::Lazy;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{PermissionState, PermissionStatus};

use crate::common::png;

// Did we get the permissions?
static CLIPBOARD_READ: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static CLIPBOARD_WRITE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

fn permission(name: &str) -> js_sys::Object {
   let obj = js_sys::Object::new();
   js_sys::Reflect::set(&obj, &"name".into(), &name.into());
   obj
}

pub fn init() -> anyhow::Result<()> {
   // Clipboard MAY work on firefox, so I'll hide option to force it
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

   let window = web_sys::window().unwrap();
   let navigator = window.navigator();
   let permissions = navigator.permissions().unwrap();

   let read = Closure::wrap(Box::new(move |status: JsValue| {
      let status = status.dyn_into::<PermissionStatus>().unwrap();
      log::info!("Clipboard read permission state: {:?}", status.state());

      use PermissionState::*;
      match status.state() {
         Granted | Prompt => CLIPBOARD_READ.store(true, Ordering::Relaxed),
         _ => (),
      }
   }) as Box<dyn FnMut(_)>);

   let write = Closure::wrap(Box::new(move |status: JsValue| {
      let status = status.dyn_into::<PermissionStatus>().unwrap();
      log::info!("Clipboard write permission state: {:?}", status.state());

      use PermissionState::*;
      match status.state() {
         Granted | Prompt => CLIPBOARD_WRITE.store(true, Ordering::Relaxed),
         _ => (),
      }
   }) as Box<dyn FnMut(_)>);

   // Query for clipboard read and write
   permissions.query(&permission("clipboard-read")).unwrap().then(&read);
   permissions.query(&permission("clipboard-write")).unwrap().then(&write);

   read.forget();
   write.forget();

   Ok(())
}

pub fn copy_string(string: String) -> anyhow::Result<()> {
   if CLIPBOARD_WRITE.load(Ordering::Relaxed) {
      let window = web_sys::window().unwrap();
      let navigator = window.navigator();
      let clipboard = navigator.clipboard().unwrap();

      clipboard.write_text(&string);

      Ok(())
   } else {
      anyhow::bail!("no permissions to copy text to clipboard")
   }
}

// web-sys doesn't offer a ClipboardItem constructor,
// so I import a function for that (and overall, that's better).
#[wasm_bindgen(raw_module = "../www/index.js")]
extern "C" {
   #[wasm_bindgen(js_name = createClipboardItem)]
   fn create_clipboard_item(mime: &str, blob: &JsValue) -> JsValue;
}

pub fn copy_image(image: RgbaImage) -> anyhow::Result<()> {
   if CLIPBOARD_WRITE.load(Ordering::Relaxed) {
      let window = web_sys::window().unwrap();
      let navigator = window.navigator();
      let clipboard = navigator.clipboard().unwrap();

      let buf = png::encode_to_vec(image)?;

      let blob = gloo_file::Blob::new_with_options(buf.as_slice(), Some("image/png"));
      clipboard.write(&create_clipboard_item("image/png", blob.as_ref()));

      Ok(())
   } else {
      anyhow::bail!("no permissions to copy image to clipboard")
   }
}

pub fn paste_string() -> anyhow::Result<String> {
   anyhow::bail!("paste_string not implemented yet")
}

pub fn paste_image() -> anyhow::Result<RgbaImage> {
   anyhow::bail!("paste_image not implemented yet")
}
