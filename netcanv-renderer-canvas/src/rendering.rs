use netcanv_renderer::paws::{self, AlignH, AlignV, Renderer};
use netcanv_renderer::{BlendMode, RenderBackend};
use std::rc::Rc;

use crate::common::*;
use crate::font::Font;
use crate::framebuffer::Framebuffer;
use crate::image::Image;
use crate::state::State;
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

   pub(crate) fn draw_image(&mut self, image: &Image, position: netcanv_renderer::paws::Rect) {
      match self.cache.get(image.data()) {
         Some(i) => {
            self.context.draw_image_with_html_image_element(
               i,
               position.x() as _,
               position.y() as _,
            );
         }
         None => {
            let i = image.build();
            self.context.draw_image_with_html_image_element(
               &i,
               position.x() as _,
               position.y() as _,
            );
            self.cache.insert(image.data().to_vec(), i);
         }
      }
   }
}

impl Renderer for CanvasBackend {
   type Font = Font;

   fn push(&mut self) {
      self.states.push(Default::default());
      self.current_state += 1;
      self.context.save();
   }

   fn pop(&mut self) {
      self.context.restore();
      self.current_state -= 1;
      self.states.pop();
   }

   fn translate(&mut self, vec: netcanv_renderer::paws::Vector) {
      self.states[self.current_state].translation = vec;
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

      let x = rect.x() as f64;
      let y = rect.y() as f64;
      let width = rect.width() as f64;
      let height = rect.height() as f64;
      let radius = radius as f64;
      let ctx = &self.context;

      ctx.begin_path();
      ctx.move_to(x + radius, y);
      ctx.line_to(x + width - radius, y);
      ctx.arc_to(x + width, y, x + width, y + radius, radius);
      ctx.line_to(x + width, y + height - radius);
      ctx.arc_to(
         x + width,
         y + height,
         x + width - radius,
         y + height,
         radius,
      );
      ctx.line_to(x + radius, y + height);
      ctx.arc_to(x, y + height, x, y + height - radius, radius);
      ctx.line_to(x, y + radius);
      ctx.arc_to(x, y, x + radius, y, radius);
      ctx.close_path();
      ctx.fill();

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
      ctx.arc_to(x + width, y, x + width, y + radius, radius);
      ctx.line_to(x + width, y + height - radius);
      ctx.arc_to(
         x + width,
         y + height,
         x + width - radius,
         y + height,
         radius,
      );
      ctx.line_to(x + radius, y + height);
      ctx.arc_to(x, y + height, x, y + height - radius, radius);
      ctx.line_to(x, y + radius);
      ctx.arc_to(x, y, x + radius, y, radius);
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

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Framebuffer {
      log::info!("create framebuffer {} {}", width, height);
      Framebuffer::new(width, height)
   }

   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self)) {
      // Get the current state, because netcanv requires renderer to have
      // global state. That doesn't works with Canvas API, so we need to do it ourselves.
      let State {
         translation,
         scaling,
      } = self.states[self.current_state];

      let framebuffer_context = framebuffer.context.take().unwrap();
      let old_context = std::mem::replace(&mut self.context, framebuffer_context);

      // Apply current state, because Canvas API doesn't provide us function to
      // get entire canvas state and set it.
      self.context.save();
      self.context.translate(translation.x as _, translation.y as _);
      self.context.scale(scaling.x as _, scaling.y as _);
      f(self);
      self.context.restore();

      let framebuffer_context = std::mem::replace(&mut self.context, old_context);
      framebuffer.context.set(Some(framebuffer_context));
   }

   fn clear(&mut self, color: netcanv_renderer::paws::Color) {
      let width = self.window.inner_size().width;
      let height = self.window.inner_size().height;

      self.push();

      self.set_fill_color(color);
      self.context.fill_rect(0.0f64, 0.0f64, width as _, height as _);

      self.pop();
   }

   fn image(&mut self, position: netcanv_renderer::paws::Rect, image: &Self::Image) {
      self.draw_image(image, position);
   }

   fn framebuffer(
      &mut self,
      position: netcanv_renderer::paws::Rect,
      framebuffer: &Self::Framebuffer,
   ) {
      // self.outline(
      //    position,
      //    paws::Color {
      //       r: 255,
      //       g: 0,
      //       b: 0,
      //       a: 255,
      //    },
      //    0.0,
      //    2.0,
      // );

      self.context.draw_image_with_html_canvas_element_and_dw_and_dh(
         &framebuffer.canvas,
         position.x() as _,
         position.y() as _,
         position.width() as _,
         position.height() as _,
      );
   }

   fn scale(&mut self, scale: netcanv_renderer::paws::Vector) {
      self.states[self.current_state].scaling = scale;
      self.context.scale(scale.x as _, scale.y as _);
   }

   fn set_blend_mode(&mut self, new_blend_mode: netcanv_renderer::BlendMode) {
      let mode = match new_blend_mode {
         BlendMode::Add => "lighter",
         BlendMode::Alpha => "source-over",
         BlendMode::Clear => "destination-out",
         BlendMode::Invert => "difference",
      };

      self.context.set_global_composite_operation(mode);
   }

   fn fill_circle(
      &mut self,
      center: netcanv_renderer::paws::Point,
      radius: f32,
      color: netcanv_renderer::paws::Color,
   ) {
      self.push();

      self.set_stroke_color(color);

      self.context.begin_path();
      self.context.arc(
         center.x as _,
         center.y as _,
         radius as _,
         0.0f64,
         2.0f64 * std::f64::consts::PI,
      );
      self.context.fill();

      self.pop();
   }

   fn outline_circle(
      &mut self,
      center: netcanv_renderer::paws::Point,
      radius: f32,
      color: netcanv_renderer::paws::Color,
      thickness: f32,
   ) {
      self.push();

      self.set_stroke_color(color);
      self.context.set_line_width(thickness as _);

      self.context.begin_path();
      self.context.arc(
         center.x as _,
         center.y as _,
         radius as _,
         0.0f64,
         2.0f64 * std::f64::consts::PI,
      );
      self.context.stroke();

      self.pop();
   }
}
