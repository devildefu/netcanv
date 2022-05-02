use js_sys::{ArrayBuffer, Uint8Array};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, FontFace};

// https://rustwasm.github.io/docs/wasm-bindgen/reference/passing-rust-closures-to-js.html#heap-allocated-closures
#[wasm_bindgen]
pub struct FontLoader {
   _closure: Closure<dyn FnMut(JsValue)>,
}

impl FontLoader {
   pub fn new<F: 'static>(font: FontFace, f: F) -> Self
   where
      F: FnMut(JsValue),
   {
      let _closure = Closure::new(f);

      font.load().unwrap().then(&_closure);

      FontLoader { _closure }
   }
}

static FONT_COUNTER: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

pub struct Font {
   normal_name: String,
   name: String,
   size: f32,
   pub(crate) context: RefCell<Option<Rc<CanvasRenderingContext2d>>>,
   _loader: Option<FontLoader>,
}

impl Font {
   pub(crate) fn from_memory(memory: &[u8], default_size: f32) -> Self {
      let buffer = ArrayBuffer::new(memory.len() as _);
      let view = Uint8Array::new(&buffer);
      view.copy_from(memory);

      // FontFace wants a family name, and current API doesn't tell me the name, so let's do it ourselves!
      let prev = FONT_COUNTER.fetch_add(1, Ordering::SeqCst);
      let normal_name = format!("netcanv-font-{}", prev);
      let font_name = format!("{}px {}", default_size, normal_name);

      // I wanted to use new_with_u8_array, but it requires &mut [u8] from me, maybe someone knows better alternative?
      // For now, I'm using ArrayBuffer
      let font = FontFace::new_with_array_buffer(&normal_name, &buffer).unwrap();

      // Add font later
      let loader = FontLoader::new(font, |font| {
         use wasm_bindgen::JsCast;

         // https://developer.mozilla.org/en-US/docs/Web/API/FontFace/FontFace#example
         let window = web_sys::window().unwrap();
         let document = window.document().unwrap();
         let fonts = document.fonts();
         let font = font.dyn_into::<FontFace>().unwrap();

         if let Err(_) = fonts.add(&font) {
            log::error!("Failed to load font");
         }
      });

      Self {
         normal_name,
         name: font_name,
         size: default_size,
         _loader: Some(loader),
         context: RefCell::new(None),
      }
   }

   /// Get a reference to the font's font name.
   pub fn name(&self) -> &str {
      self.name.as_str()
   }
}

impl netcanv_renderer::Font for Font {
   fn with_size(&self, new_size: f32) -> Self {
      // Canvas API font property is just name, so we just need to copy everything and change size
      Self {
         name: format!("{}px {}", new_size, self.normal_name),
         normal_name: self.normal_name.clone(),
         size: new_size,
         _loader: None,
         context: self.context.clone(),
      }
   }

   fn size(&self) -> f32 {
      self.size
   }

   fn height(&self) -> f32 {
      self.size
   }

   fn text_width(&self, text: &str) -> f32 {
      let context = self.context.borrow();
      if let Some(c) = &*context {
         c.save();

         c.set_font(&self.name);
         let metrics = c.measure_text(text).unwrap();

         c.restore();

         metrics.width() as _
      } else {
         log::error!("Attempt to measure text width before using Font (context is None)");
         0.0f32
      }
   }
}
