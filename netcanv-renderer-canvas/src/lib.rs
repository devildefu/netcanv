use std::{
   cell::RefCell,
   rc::Rc,
   sync::atomic::{AtomicUsize, Ordering},
};

use js_sys::{ArrayBuffer, Uint8Array};

use netcanv_renderer::{
   paws::{AlignH, AlignV, Renderer, Ui},
   RenderBackend,
};

use once_cell::sync::Lazy;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, FontFace, ImageData};
use winit::{
   event_loop::EventLoop,
   platform::web::WindowExtWebSys,
   window::{Window, WindowBuilder},
};

// https://rustwasm.github.io/docs/wasm-bindgen/reference/passing-rust-closures-to-js.html#heap-allocated-closures
#[wasm_bindgen]
pub struct FontLoader {
   closure: Closure<dyn FnMut(JsValue)>,
}

impl FontLoader {
   pub fn new<F: 'static>(font: FontFace, f: F) -> Self
   where
      F: FnMut(JsValue),
   {
      let closure = Closure::new(f);

      font.load().unwrap().then(&closure);

      FontLoader { closure }
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
   /// Get a reference to the font's font name.
   pub fn name(&self) -> &str {
      self.name.as_str()
   }
}

impl netcanv_renderer::Font for Font {
   fn from_memory(memory: &[u8], default_size: f32) -> Self {
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
      todo!()
   }

   fn text_width(&self, text: &str) -> f32 {
      let context = self.context.borrow();
      if let Some(c) = &*context {
         let metrics = c.measure_text(text).unwrap();
         metrics.width() as _
      } else {
         log::error!("Attempt to measure text width before using Font (context is None)");
         0.0f32
      }
   }
}

pub struct Image {
   image_data: ImageData,
   data: Vec<u8>,
}

impl Image {
   /// Get a reference to the image's image data.
   pub fn image_data(&self) -> &ImageData {
      &self.image_data
   }
}

impl netcanv_renderer::Image for Image {
   fn from_rgba(width: u32, height: u32, pixel_data: &[u8]) -> Self {
      let data = pixel_data.to_vec();

      let image_data =
         ImageData::new_with_u8_clamped_array_and_sh(Clamped(data.as_slice()), width, height)
            .unwrap();

      Self { data, image_data }
   }

   fn colorized(&self, color: netcanv_renderer::paws::Color) -> Self {
      let mut data = self.data.clone();

      for pixel in data.chunks_mut(4) {
         pixel[0] = color.r;
         pixel[1] = color.g;
         pixel[2] = color.b;
         pixel[3] = ((pixel[3] as f32 / 255.0) * (color.a as f32 / 255.0) * 255.0) as u8;
      }

      let image_data = ImageData::new_with_u8_clamped_array_and_sh(
         Clamped(data.as_slice()),
         self.image_data.width(),
         self.image_data.height(),
      )
      .unwrap();

      Self { data, image_data }
   }

   fn size(&self) -> (u32, u32) {
      (self.image_data.width(), self.image_data.height())
   }
}

pub struct Framebuffer {}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      todo!()
   }

   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {
      todo!()
   }

   fn download_rgba(&self, dest: &mut [u8]) {
      todo!()
   }
}

pub struct CanvasBackend {
   context: Rc<web_sys::CanvasRenderingContext2d>,
   window: winit::window::Window,
}

impl CanvasBackend {
   pub fn new(window_builder: WindowBuilder, event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
      use wasm_bindgen::JsCast;

      let winit_window = window_builder.build(&event_loop)?;
      let canvas = winit_window.canvas();
      let window = web_sys::window().unwrap();
      let document = window.document().unwrap();
      let body = document.body().unwrap();
      body.append_child(&canvas).expect("Append canvas to HTML body");

      let context = canvas
         .get_context("2d")
         .unwrap()
         .unwrap()
         .dyn_into::<web_sys::CanvasRenderingContext2d>()
         .unwrap();

      Ok(Self {
         context: Rc::new(context),
         window: winit_window,
      })
   }

   pub fn window(&self) -> &Window {
      &self.window
   }

   // TODO: Handle errors
   #[allow(dead_code)]
   pub(crate) fn font_exists(name: &str) -> bool {
      let document = web_sys::window().unwrap().document().unwrap();
      let fonts = document.fonts();
      fonts.check(&format!("12px {}", name)).unwrap()
   }

   pub(crate) fn set_color(&mut self, color: netcanv_renderer::paws::Color) {
      self.context.set_fill_style(&JsValue::from_str(&format!(
         "rgb({}, {}, {})",
         color.r, color.g, color.b
      )));
   }

   pub(crate) fn draw_image(&mut self, image: &Image, position: netcanv_renderer::paws::Point) {
      if let Err(e) =
         self.context.put_image_data(image.image_data(), position.x as _, position.y as _)
      {
         log::error!("jeblo");
      }
   }
}

impl Renderer for CanvasBackend {
   type Font = Font;

   fn push(&mut self) {
      self.context.save();
   }

   fn pop(&mut self) {
      self.context.restore();
   }

   fn translate(&mut self, vec: netcanv_renderer::paws::Vector) {
      self.context.translate(vec.x as _, vec.y as _);
   }

   fn clip(&mut self, rect: netcanv_renderer::paws::Rect) {
      let path2d = web_sys::Path2d::new().unwrap();

      path2d.rect(
         rect.x() as _,
         rect.y() as _,
         rect.width() as _,
         rect.height() as _,
      );

      self.context.clip_with_path_2d(&path2d);
   }

   fn fill(
      &mut self,
      rect: netcanv_renderer::paws::Rect,
      color: netcanv_renderer::paws::Color,
      _radius: f32,
   ) {
      self.set_color(color);

      self.context.fill_rect(
         rect.x() as _,
         rect.y() as _,
         rect.width() as _,
         rect.height() as _,
      );
   }

   fn outline(
      &mut self,
      rect: netcanv_renderer::paws::Rect,
      color: netcanv_renderer::paws::Color,
      _radius: f32,
      thickness: f32,
   ) {
      self.set_color(color);
      self.context.set_line_width(thickness as _);

      let x = if rect.x() % 2.0f32 > 0.95f32 {
         rect.x() + 0.5f32
      } else {
         rect.x()
      };

      let y = if rect.y() % 2.0f32 > 0.95f32 {
         rect.y() + 0.5f32
      } else {
         rect.y()
      };

      self.context.stroke_rect(x as _, y as _, rect.width() as _, rect.height() as _);
   }

   fn line(
      &mut self,
      a: netcanv_renderer::paws::Point,
      b: netcanv_renderer::paws::Point,
      color: netcanv_renderer::paws::Color,
      cap: netcanv_renderer::paws::LineCap,
      thickness: f32,
   ) {
      use netcanv_renderer::paws::LineCap;

      self.set_color(color);
      self.context.set_line_width(thickness as _);
      self.context.set_line_cap(match cap {
         LineCap::Butt => "butt",
         LineCap::Round => "round",
         LineCap::Square => "square",
      });

      self.context.move_to(a.x as _, a.y as _);
      self.context.line_to(b.x as _, b.y as _);
   }

   fn text(
      &mut self,
      rect: netcanv_renderer::paws::Rect,
      font: &Self::Font,
      text: &str,
      color: netcanv_renderer::paws::Color,
      alignment: netcanv_renderer::paws::Alignment,
   ) -> f32 {
      if font.context.borrow().is_none() {
         *font.context.borrow_mut() = Some(Rc::clone(&self.context));
      }

      self.set_color(color);

      let (align, x) = match alignment {
         (AlignH::Left, _) => ("left", rect.left()),
         (AlignH::Center, _) => ("center", rect.center_x()),
         (AlignH::Right, _) => ("right", rect.right()),
      };

      let (baseline, y) = match alignment {
         (_, AlignV::Top) => ("top", rect.top()),
         (_, AlignV::Middle) => ("middle", rect.center_y()),
         (_, AlignV::Bottom) => ("bottom", rect.bottom()),
      };

      self.context.set_text_align(align);
      self.context.set_text_baseline(baseline);
      self.context.set_font(font.name());
      self.context.fill_text(text, x as _, y as _);

      let metrics = self.context.measure_text(text).unwrap();

      metrics.width() as _
   }
}

impl RenderBackend for CanvasBackend {
   type Image = Image;
   type Framebuffer = Framebuffer;

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      todo!()
   }

   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self)) {
      todo!()
   }

   fn clear(&mut self, color: netcanv_renderer::paws::Color) {
      let width = self.window.inner_size().width;
      let height = self.window.inner_size().height;
      self.set_color(color);
      self.context.fill_rect(0.0f64, 0.0f64, width as _, height as _);
   }

   fn image(&mut self, position: netcanv_renderer::paws::Point, image: &Self::Image) {
      self.draw_image(image, position);
   }

   fn framebuffer(
      &mut self,
      position: netcanv_renderer::paws::Point,
      framebuffer: &Self::Framebuffer,
   ) {
      todo!()
   }

   fn scale(&mut self, scale: netcanv_renderer::paws::Vector) {
      todo!()
   }

   fn set_blend_mode(&mut self, new_blend_mode: netcanv_renderer::BlendMode) {
      todo!()
   }

   fn fill_circle(
      &mut self,
      center: netcanv_renderer::paws::Point,
      radius: f32,
      color: netcanv_renderer::paws::Color,
   ) {
      todo!()
   }

   fn outline_circle(
      &mut self,
      center: netcanv_renderer::paws::Point,
      radius: f32,
      color: netcanv_renderer::paws::Color,
      thickness: f32,
   ) {
      todo!()
   }
}

pub trait UiRenderFrame {
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<CanvasBackend> {
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      callback(self);
      self.window.request_redraw();
      Ok(())
   }
}
