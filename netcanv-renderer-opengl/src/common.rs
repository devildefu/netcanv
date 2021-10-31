use glow::HasContext;
use netcanv_renderer::paws::{vector, Color, Vector};

pub fn normalized_color(color: Color) -> (f32, f32, f32, f32) {
   (
      color.r as f32 / 255.0,
      color.g as f32 / 255.0,
      color.b as f32 / 255.0,
      color.a as f32 / 255.0,
   )
}

pub trait VectorMath {
   fn length(self) -> f32;
   fn normalize(self) -> Self;
   fn perpendicular_cw(self) -> Self;
   fn perpendicular_ccw(self) -> Self;
}

impl VectorMath for Vector {
   fn length(self) -> f32 {
      (self.x * self.x + self.y * self.y).sqrt()
   }

   fn normalize(self) -> Self {
      let length = self.length();
      if length == 0.0 {
         vector(0.0, 0.0)
      } else {
         self / length
      }
   }

   fn perpendicular_cw(self) -> Self {
      vector(-self.y, self.x)
   }

   fn perpendicular_ccw(self) -> Self {
      vector(self.y, -self.x)
   }
}

pub trait GlUtilities {
   unsafe fn texture_swizzle_mask(&self, target: u32, mask: &[u32; 4]);
}

impl GlUtilities for glow::Context {
   unsafe fn texture_swizzle_mask(&self, target: u32, mask: &[u32; 4]) {
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_R, mask[0] as i32);
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_G, mask[1] as i32);
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_B, mask[2] as i32);
      self.tex_parameter_i32(target, glow::TEXTURE_SWIZZLE_A, mask[3] as i32);
   }
}
