use netcanv_renderer::paws;
use wasm_bindgen::JsValue;

pub fn color_to_jsvalue(color: paws::Color) -> JsValue {
   JsValue::from_str(&format!(
      "rgba({}, {}, {}, {})",
      color.r,
      color.g,
      color.b,
      color.a as f32 / 255.0
   ))
}
