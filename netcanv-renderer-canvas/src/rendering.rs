use netcanv_renderer::{
   paws::{AlignH, AlignV, Renderer},
   RenderBackend,
};
use std::rc::Rc;

use crate::common::*;
use crate::font::Font;
use crate::framebuffer::Framebuffer;
use crate::image::Image;
use crate::CanvasBackend;

impl CanvasBackend {
   // TODO: Handle errors
   #[allow(dead_code)]
   pub(crate) fn font_exists(name: &str) -> bool {
      let document = web_sys::window().unwrap().document().unwrap();
      let fonts = document.fonts();
      fonts.check(&format!("12px {}", name)).unwrap()
   }

   pub(crate) fn set_stroke_color(&mut self, color: netcanv_renderer::paws::Color) {
      self.context.set_stroke_style(&color_to_jsvalue(color));
   }

   pub(crate) fn set_fill_color(&mut self, color: netcanv_renderer::paws::Color) {
      self.context.set_fill_style(&color_to_jsvalue(color));
   }

   pub(crate) fn draw_image(&mut self, image: &Image, position: netcanv_renderer::paws::Point) {
      match self.cache.get(image.data()) {
         Some(i) => {
            self.context.draw_image_with_html_image_element(i, position.x as _, position.y as _);
         }
         None => {
            let i = image.build();
            self.context.draw_image_with_html_image_element(&i, position.x as _, position.y as _);
            self.cache.insert(image.data().to_vec(), i);
         }
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
      radius: f32,
   ) {
      self.push();

      self.set_fill_color(color);

      if radius > 0.0f32 {
         self.context.set_line_join("round");
         self.context.set_line_width(radius as _);
      }

      self.context.fill_rect(
         rect.x() as _,
         rect.y() as _,
         rect.width() as _,
         rect.height() as _,
      );

      self.pop();
   }

   fn outline(
      &mut self,
      rect: netcanv_renderer::paws::Rect,
      color: netcanv_renderer::paws::Color,
      radius: f32,
      thickness: f32,
   ) {
      self.push();

      self.set_stroke_color(color);
      self.context.set_line_width(thickness as _);

      if thickness % 2.0 > 0.95 {
         self.context.translate(0.5, 0.5);
      }

      let x = rect.x() as f64;
      let y = rect.y() as f64;
      let width = rect.width() as f64;
      let height = rect.height() as f64;
      let radius = radius as f64;
      let ctx = &self.context;

      ctx.begin_path();
      ctx.move_to(x + radius, y);
      ctx.line_to(x + width - radius, y);
      ctx.quadratic_curve_to(x + width, y, x + width, y + radius);
      ctx.line_to(x + width, y + height - radius);
      ctx.quadratic_curve_to(x + width, y + height, x + width - radius, y + height);
      ctx.line_to(x + radius, y + height);
      ctx.quadratic_curve_to(x, y + height, x, y + height - radius);
      ctx.line_to(x, y + radius);
      ctx.quadratic_curve_to(x, y, x + radius, y);
      ctx.close_path();
      ctx.stroke();

      self.pop();
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

      self.push();

      self.set_stroke_color(color);
      self.context.set_line_width(thickness as _);
      self.context.set_line_cap(match cap {
         LineCap::Butt => "butt",
         LineCap::Round => "round",
         LineCap::Square => "square",
      });

      self.context.begin_path();
      self.context.move_to(a.x as _, a.y as _);
      self.context.line_to(b.x as _, b.y as _);
      self.context.stroke();

      self.pop();
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

      self.push();

      self.set_fill_color(color);

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

      self.pop();

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

      self.push();

      self.set_fill_color(color);
      self.context.fill_rect(0.0f64, 0.0f64, width as _, height as _);

      self.pop();
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
