use gloo_storage::{LocalStorage, Storage};
use image::{load_from_memory_with_format, ImageFormat, RgbaImage};
use js_sys::{Array, JsString, Object, Reflect, Uint8Array};
use once_cell::sync::Lazy;
use std::{
   str::FromStr,
   sync::atomic::{AtomicBool, Ordering},
};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, Clipboard, ClipboardItem};

use wasm_bindgen::prelude::*;

use crate::{common::get_from_js_value, image_coder::ImageCoder};

static CLIPBOARD_READ: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static CLIPBOARD_WRITE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

// `web-sys` doesn't provide `ClipboardItem::new()`, and `ClipboardItem::from()` casts JsValue to
// ClipboardItem. So we use `wasm-bindgen`'s inline JS feature to create one line wrapper for ClipboardItem
// constructor, which we will use to call ClipboardItem's constructor
#[wasm_bindgen(
   inline_js = "export function new_clipboard_item(data) { return new ClipboardItem(data); }"
)]
extern "C" {
   fn new_clipboard_item(data: Object) -> ClipboardItem;
}

fn get_clipboard() -> Option<Clipboard> {
   web_sys::window()?.navigator().clipboard()
}

async fn ask_for_permission(name: &str) -> Result<bool, JsValue> {
   // Create object with property "name", because Permission.query() usage is:
   // permissions.query({ name: "value" })
   let name = JsString::from_str(name).unwrap();
   let object = Object::new();
   Reflect::set(&object, &JsString::from_str("name").unwrap(), &name)?;

   let window = web_sys::window().unwrap();
   let navigator = window.navigator();

   if let Ok(permissions) = navigator.permissions() {
      let promise = permissions.query(&object)?;
      let permission = JsFuture::from(promise).await?;
      let state = get_from_js_value(&permission, "state")?.as_string().unwrap();

      Ok(state == "granted" || state == "prompt")
   } else {
      Ok(false)
   }
}

pub fn init() -> netcanv::Result<()> {
   match LocalStorage::get("_FORCE_CLIPBOARD") {
      Ok(v) if v => {
         log::info!("Forced clipboard");

         CLIPBOARD_READ.store(true, Ordering::Relaxed);
         CLIPBOARD_WRITE.store(true, Ordering::Relaxed);

         return Ok(());
      }
      _ => (),
   }

   match LocalStorage::get("_ASK_FOR_PERMISSIONS") {
      Ok(v) if v => {
         wasm_bindgen_futures::spawn_local(async {
            let read = ask_for_permission("clipboard-read").await.unwrap();
            log::info!("clipboard-read: {}", read);
            CLIPBOARD_READ.store(read, Ordering::Relaxed);

            let write = ask_for_permission("clipboard-write").await.unwrap();
            log::info!("clipboard-write: {}", write);
            CLIPBOARD_WRITE.store(write, Ordering::Relaxed);
         });
      }
      _ => (),
   }

   Ok(())
}

pub fn copy_string(string: String) -> netcanv::Result<()> {
   if CLIPBOARD_WRITE.load(Ordering::Relaxed) {
      wasm_bindgen_futures::spawn_local(async move {
         let clipboard = get_clipboard().unwrap();
         JsFuture::from(clipboard.write_text(&string)).await.unwrap();
      });
      Ok(())
   } else {
      Err(netcanv::Error::ClipboardUnknown {
         error: "no permissions to copy text to clipboard".to_string(),
      })
   }
}

pub fn copy_image(image: RgbaImage) -> netcanv::Result<()> {
   if CLIPBOARD_WRITE.load(Ordering::Relaxed) {
      // Encode image into Blob
      let buf = ImageCoder::encode_png_data(image)?;
      let blob = gloo_file::Blob::new_with_options(buf.as_slice(), Some("image/png"));

      // Create ClipboardItem
      let item = Object::new();
      let key = JsString::from_str("image/png").unwrap();
      let value: &JsValue = blob.as_ref();
      Reflect::set(&item, &key, value).unwrap();
      let item = new_clipboard_item(item);

      // Make array from ClipboardItem
      let data = Array::of1(&item);

      wasm_bindgen_futures::spawn_local(async move {
         let clipboard = get_clipboard().unwrap();
         JsFuture::from(clipboard.write(&data)).await.unwrap();
      });

      Ok(())
   } else {
      Err(netcanv::Error::ClipboardUnknown {
         error: "no permissions to copy image to clipboard".to_string(),
      })
   }
}

pub fn paste_string<F>(func: F)
where
   F: FnOnce(netcanv::Result<String>) + 'static,
{
   wasm_bindgen_futures::spawn_local(async {
      if CLIPBOARD_READ.load(Ordering::Relaxed) {
         let clipboard = get_clipboard().unwrap();
         let content = JsFuture::from(clipboard.read_text()).await.unwrap();
         let string = content.as_string().unwrap();

         func(Ok(string));
      } else {
         func(Err(netcanv::Error::ClipboardUnknown {
            error: "no permissions to paste image from clipboard".to_string(),
         }));
      }
   });
}

pub fn paste_image<F>(func: F)
where
   F: FnOnce(netcanv::Result<RgbaImage>) + 'static,
{
   wasm_bindgen_futures::spawn_local(async {
      if CLIPBOARD_READ.load(Ordering::Relaxed) {
         let clipboard = get_clipboard().unwrap();

         let contents = JsFuture::from(clipboard.read()).await.unwrap();
         let iterator = js_sys::try_iter(&contents).unwrap().unwrap();

         for item in iterator {
            let item = item.unwrap();
            let item = JsCast::unchecked_ref::<ClipboardItem>(&item);
            let types = item.types();

            if let Some(_) = types.iter().find(|x| x == "image/png") {
               let blob: Blob =
                  JsFuture::from(item.get_type("image/png")).await.unwrap().unchecked_into();
               let buffer = JsFuture::from(blob.array_buffer()).await.unwrap();

               let bytes = Uint8Array::new(&buffer).to_vec();
               func(Ok(load_from_memory_with_format(&bytes, ImageFormat::Png)
                  .unwrap()
                  .to_rgba8()));

               // We paste only first image
               break;
            }
         }
      } else {
         func(Err(netcanv::Error::ClipboardUnknown {
            error: "no permissions to paste image from clipboard".to_string(),
         }));
      }
   });
}
