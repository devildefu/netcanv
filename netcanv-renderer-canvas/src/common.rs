use netcanv_renderer::paws;

use wasm_bindgen::JsValue;
use winit::dpi::LogicalSize;

pub fn color_to_jsvalue(color: paws::Color) -> JsValue {
   JsValue::from_str(&format!(
      "rgba({}, {}, {}, {})",
      color.r,
      color.g,
      color.b,
      color.a as f32 / 255.0
   ))
}

pub fn get_window_size() -> LogicalSize<u32> {
   let window = web_sys::window().unwrap();

   let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
   let height = window.inner_height().unwrap().as_f64().unwrap() as u32;

   LogicalSize::new(width, height)
}

pub mod webp {
   use wasm_bindgen::JsCast;
   use web_sys::{HtmlCanvasElement, HtmlImageElement};

   /// Uses browser to decode webp to raw pixels. First it creates HtmlImageElement,
   /// and loads image into it. Then it creates new canvas, draws image to canvas,
   /// then gets pixels from canvas and passes them forward.
   pub fn decode((width, height): (u32, u32), data: &[u8]) -> Option<Vec<u8>> {
      let image = HtmlImageElement::new_with_width_and_height(width, height).unwrap();

      let base64 = format!("data:image/webp;base64,{}", base64::encode(&data));
      image.set_src(&base64);

      let window = web_sys::window().unwrap();
      let document = window.document().unwrap();
      let canvas =
         document.create_element("canvas").unwrap().dyn_into::<HtmlCanvasElement>().unwrap();

      canvas.set_width(width);
      canvas.set_height(height);

      let context = canvas
         .get_context("2d")
         .unwrap()
         .unwrap()
         .dyn_into::<web_sys::CanvasRenderingContext2d>()
         .unwrap();

      context.draw_image_with_html_image_element(&image, 0.0, 0.0);

      let image_data = context.get_image_data(0.0, 0.0, width as _, height as _).unwrap();
      let data = image_data.data();

      Some(data.0)
   }
}
