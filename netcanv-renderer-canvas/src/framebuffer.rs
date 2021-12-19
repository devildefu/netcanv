use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use once_cell::sync::Lazy;

use wasm_bindgen::{Clamped, JsCast};
use web_sys::ImageData;

pub struct Framebuffer {
   width: u32,
   height: u32,
   pub(crate) canvas: web_sys::HtmlCanvasElement,
   pub(crate) context: Cell<Option<Rc<web_sys::CanvasRenderingContext2d>>>,
}

impl Framebuffer {
   pub(crate) fn new(width: u32, height: u32) -> Self {
      let window = web_sys::window().unwrap();
      let document = window.document().unwrap();
      let canvas = document
         .create_element("canvas")
         .unwrap()
         .dyn_into::<web_sys::HtmlCanvasElement>()
         .unwrap();

      canvas.set_width(width);
      canvas.set_height(height);

      let context = canvas
         .get_context("2d")
         .unwrap()
         .unwrap()
         .dyn_into::<web_sys::CanvasRenderingContext2d>()
         .unwrap();

      Self {
         width,
         height,
         canvas,
         context: Cell::new(Some(Rc::new(context))),
      }
   }
}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }

   fn upload_rgba(&mut self, (x, y): (u32, u32), (width, height): (u32, u32), pixels: &[u8]) {
      assert!(
         pixels.len() == width as usize * height as usize * 4,
         "input pixel data size does not match the provided dimensions"
      );

      let image_data =
         ImageData::new_with_u8_clamped_array_and_sh(Clamped(pixels), width, height).unwrap();

      self.context.get_mut().as_mut().unwrap().put_image_data(&image_data, x as _, y as _);
   }

   fn download_rgba(&self, dest: &mut [u8]) {
      let mut context = self.context.take();

      let image_data = context
         .as_mut()
         .unwrap()
         .get_image_data(0.0, 0.0, self.width as _, self.height as _)
         .unwrap();

      let data = image_data.data().0;

      dest.copy_from_slice(&data);

      self.context.set(context);
   }
}
