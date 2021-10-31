// Honestly, I don't like this code a lot…
// There's tons of side effects, which stem from OpenGL's statefullness.
// Most things are abstracted away such that only a few specific functions need to be called to
// draw things, so it shouldn't be _that_ horrible.

use std::mem::size_of;
use std::rc::Rc;

use glow::{Buffer, HasContext, Program, Shader, Texture, UniformLocation, VertexArray};
use memoffset::offset_of;
use netcanv_renderer::paws::{
   point, vector, Alignment, Color, LineCap, Point, Rect, Renderer, Vector,
};
use netcanv_renderer::{BlendMode, Image as ImageTrait, RenderBackend};

use crate::common::{normalized_color, GlUtilities, VectorMath};
use crate::font::Font;
use crate::framebuffer::Framebuffer;
use crate::image::Image;
use crate::OpenGlBackend;

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vertex {
   position: Point,
   uv: Point,
   color: (f32, f32, f32, f32),
}

impl Vertex {
   fn colored(position: Point, color: Color) -> Self {
      Self {
         position,
         uv: point(0.0, 0.0),
         color: normalized_color(color),
      }
   }

   fn textured_colored(position: Point, uv: Point, color: Color) -> Self {
      Self {
         position,
         uv,
         color: normalized_color(color),
      }
   }
}

struct Uniforms {
   projection: UniformLocation,
   the_texture: UniformLocation,
}

#[derive(Clone, Copy)]
struct Transform {
   translation: Vector,
   blend_mode: BlendMode,
}

pub(crate) struct RenderState {
   gl: Rc<glow::Context>,
   vao: VertexArray,
   vbo: Buffer,
   vbo_size: usize,
   ebo: Buffer,
   ebo_size: usize,
   program: Program,
   uniforms: Uniforms,
   null_texture: Texture,
   stack: Vec<Transform>,
}

impl RenderState {
   fn create_vao(gl: &glow::Context, vbo: Buffer, ebo: Buffer) -> VertexArray {
      unsafe {
         let vao = gl.create_vertex_array().unwrap();
         gl.bind_vertex_array(Some(vao));
         gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
         gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
         let stride = size_of::<Vertex>() as i32;
         gl.vertex_attrib_pointer_f32(
            0,                                   // index
            2,                                   // size
            glow::FLOAT,                         // type
            false,                               // normalize
            stride,                              // stride
            offset_of!(Vertex, position) as i32, // offset
         );
         gl.vertex_attrib_pointer_f32(
            1,                             // index
            2,                             // size
            glow::FLOAT,                   // type
            false,                         // normalize
            stride,                        // stride
            offset_of!(Vertex, uv) as i32, // offset
         );
         gl.vertex_attrib_pointer_f32(
            2,                                // index
            4,                                // size
            glow::FLOAT,                      // type
            false,                            // normalize
            stride,                           // stride
            offset_of!(Vertex, color) as i32, // offset
         );
         gl.enable_vertex_attrib_array(0);
         gl.enable_vertex_attrib_array(1);
         gl.enable_vertex_attrib_array(2);
         vao
      }
   }

   fn create_vbo_and_ebo(gl: &glow::Context) -> (Buffer, Buffer) {
      unsafe {
         let vbo = gl.create_buffer().unwrap();
         let ebo = gl.create_buffer().unwrap();
         gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
         gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
         (vbo, ebo)
      }
   }

   fn compile_shader(gl: &glow::Context, kind: u32, source: &str) -> Result<Shader, String> {
      unsafe {
         let shader = gl.create_shader(kind)?;
         gl.shader_source(shader, source);
         gl.compile_shader(shader);
         if !gl.get_shader_compile_status(shader) {
            Err(gl.get_shader_info_log(shader))
         } else {
            Ok(shader)
         }
      }
   }

   fn create_program(gl: &glow::Context) -> (Program, Uniforms) {
      const VERTEX_SHADER: &str = r#"#version 300 es

         precision mediump float;

         layout (location = 0) in vec2 position;
         layout (location = 1) in vec2 uv;
         layout (location = 2) in vec4 color;

         uniform mat3 projection;

         out vec2 vertex_uv;
         out vec4 vertex_color;

         void main(void)
         {
            vec3 transformed_position = vec3(position, 1.0) * projection;
            gl_Position = vec4(transformed_position, 1.0);
            vertex_uv = uv;
            vertex_color = color;
         }
      "#;
      const FRAGMENT_SHADER: &str = r#"#version 300 es

         precision mediump float;

         in vec2 vertex_uv;
         in vec4 vertex_color;

         uniform sampler2D the_texture;

         out vec4 fragment_color;

         void main(void)
         {
            fragment_color = vertex_color * texture(the_texture, vertex_uv);
         }
      "#;
      unsafe {
         let vertex_shader = Self::compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER).unwrap();
         let fragment_shader =
            Self::compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER).unwrap();

         gl.shader_source(vertex_shader, VERTEX_SHADER);
         gl.compile_shader(vertex_shader);
         gl.shader_source(fragment_shader, FRAGMENT_SHADER);
         gl.compile_shader(fragment_shader);

         let program = gl.create_program().unwrap();
         gl.attach_shader(program, vertex_shader);
         gl.attach_shader(program, fragment_shader);
         gl.link_program(program);

         gl.delete_shader(vertex_shader);
         gl.delete_shader(fragment_shader);

         gl.use_program(Some(program));

         let uniforms = Uniforms {
            projection: gl.get_uniform_location(program, "projection").unwrap(),
            the_texture: gl.get_uniform_location(program, "the_texture").unwrap(),
         };
         gl.uniform_1_i32(Some(&uniforms.the_texture), 0);

         (program, uniforms)
      }
   }

   fn create_null_texture(gl: &glow::Context) -> Texture {
      unsafe {
         let texture = gl.create_texture().unwrap();
         gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            1,
            1,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            Some(&[255, 255, 255, 255]),
         );
         texture
      }
   }

   pub(crate) fn new(gl: Rc<glow::Context>) -> Self {
      let (vbo, ebo) = Self::create_vbo_and_ebo(&gl);
      let vao = Self::create_vao(&gl, vbo, ebo);
      let (program, uniforms) = Self::create_program(&gl);
      let null_texture = Self::create_null_texture(&gl);

      unsafe {
         gl.enable(glow::BLEND);
         gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
         gl.blend_func_separate(
            glow::SRC_ALPHA,
            glow::ONE_MINUS_SRC_ALPHA,
            glow::ONE,
            glow::ONE_MINUS_SRC_ALPHA,
         );
      }

      Self {
         gl,
         vao,
         vbo,
         vbo_size: 0,
         ebo,
         ebo_size: 0,
         program,
         uniforms,
         null_texture,
         stack: vec![Transform {
            translation: vector(0.0, 0.0),
            blend_mode: BlendMode::Alpha,
         }],
      }
   }

   unsafe fn to_u8_slice<T>(slice: &[T]) -> &[u8] {
      let ptr = slice.as_ptr() as *const u8;
      std::slice::from_raw_parts(ptr, size_of::<T>() * slice.len())
   }

   fn bind_null_texture(&mut self) {
      unsafe {
         self.gl.active_texture(glow::TEXTURE0);
         self.gl.bind_texture(glow::TEXTURE_2D, Some(self.null_texture));
      }
   }

   fn draw(&mut self, vertices: &[Vertex], indices: &[u32]) {
      unsafe {
         // Update buffers
         let vertex_data = Self::to_u8_slice(vertices);
         let index_data = Self::to_u8_slice(indices);
         if vertex_data.len() > self.vbo_size {
            self.gl.buffer_data_size(
               glow::ARRAY_BUFFER,
               vertex_data.len() as i32,
               glow::STREAM_DRAW,
            );
            self.vbo_size = vertex_data.len();
         }
         if index_data.len() > self.ebo_size {
            self.gl.buffer_data_size(
               glow::ELEMENT_ARRAY_BUFFER,
               index_data.len() as i32,
               glow::STREAM_DRAW,
            );
            self.ebo_size = index_data.len();
         }
         self.gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, vertex_data);
         self.gl.buffer_sub_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, 0, index_data);
         // Draw triangles
         self.gl.draw_elements(glow::TRIANGLES, indices.len() as i32, glow::UNSIGNED_INT, 0);
      }
   }

   pub(crate) fn viewport(&mut self, width: u32, height: u32) {
      let (fwidth, fheight) = (width as f32, height as f32);
      #[rustfmt::skip]
      let matrix: [f32; 3 * 3] = [
         2.0 / fwidth, 0.0,            -1.0,
         0.0,          2.0 / -fheight,  1.0,
         0.0,          0.0,             1.0,
      ];
      unsafe {
         self.gl.viewport(0, 0, width as i32, height as i32);
         self.gl.scissor(0, 0, width as i32, height as i32);
         self.gl.uniform_matrix_3_f32_slice(Some(&self.uniforms.projection), false, &matrix);
      }
   }

   fn transform(&self) -> &Transform {
      self.stack.last().unwrap()
   }

   fn transform_mut(&mut self) -> &mut Transform {
      self.stack.last_mut().unwrap()
   }
}

impl Drop for RenderState {
   fn drop(&mut self) {
      unsafe {
         self.gl.delete_buffer(self.vbo);
         self.gl.delete_buffer(self.ebo);
         self.gl.delete_vertex_array(self.vao);
         self.gl.delete_program(self.program);
      }
   }
}

impl Renderer for OpenGlBackend {
   type Font = Font;

   fn push(&mut self) {
      self.state.stack.push(self.state.transform().clone());
   }

   fn pop(&mut self) {
      self.state.stack.pop();
      assert!(
         self.state.stack.len() > 0,
         "pop() called at the bottom of the stack"
      );
   }

   fn translate(&mut self, vec: Vector) {
      self.state.transform_mut().translation += vec;
   }

   fn clip(&mut self, rect: Rect) {}

   fn fill(&mut self, mut rect: Rect, color: Color, radius: f32) {
      rect.position += self.state.transform().translation;
      let vertices = [
         Vertex::colored(rect.top_left(), color),     // 0
         Vertex::colored(rect.top_right(), color),    // 1
         Vertex::colored(rect.bottom_right(), color), // 2
         Vertex::colored(rect.bottom_left(), color),  // 3
      ];
      let indices = [0, 1, 2, 2, 3, 0];
      self.state.bind_null_texture();
      self.state.draw(&vertices, &indices);
   }

   fn outline(&mut self, mut rect: Rect, color: Color, radius: f32, thickness: f32) {
      rect.position += self.state.transform().translation;
      if thickness % 2.0 > 0.95 {
         rect.position += vector(0.5, 0.5);
      }
      let d = thickness / 2.0;
      let vertices = [
         Vertex::colored(rect.top_left() - vector(d, d), color), // 0
         Vertex::colored(rect.top_left() + vector(d, d), color), // 1
         Vertex::colored(rect.top_right() - vector(-d, d), color), // 2
         Vertex::colored(rect.top_right() + vector(-d, d), color), // 3
         Vertex::colored(rect.bottom_right() - vector(-d, -d), color), // 4
         Vertex::colored(rect.bottom_right() + vector(-d, -d), color), // 5
         Vertex::colored(rect.bottom_left() - vector(d, -d), color), // 6
         Vertex::colored(rect.bottom_left() + vector(d, -d), color), // 7
      ];
      #[rustfmt::skip]
      let indices = [
         // top edge
         0, 1, 2, 2, 3, 1,
         // right edge
         2, 3, 4, 4, 5, 3,
         // bottom edge
         4, 5, 6, 6, 7, 5,
         // left edge
         6, 7, 0, 0, 1, 7,
      ];
      self.state.bind_null_texture();
      self.state.draw(&vertices, &indices);
   }

   fn line(&mut self, mut a: Point, mut b: Point, color: Color, cap: LineCap, thickness: f32) {
      a += self.state.transform().translation;
      b += self.state.transform().translation;
      if thickness % 2.0 > 0.95 {
         a += vector(0.5, 0.5);
         b += vector(0.5, 0.5);
      }
      let direction = (b - a).normalize();
      let cw = direction.perpendicular_cw() * thickness / 2.0;
      let ccw = direction.perpendicular_ccw() * thickness / 2.0;
      let vertices = [
         Vertex::colored(a + cw, color),
         Vertex::colored(a + ccw, color),
         Vertex::colored(b + ccw, color),
         Vertex::colored(b + cw, color),
      ];
      let indices = [0, 1, 2, 2, 3, 0];
      self.state.bind_null_texture();
      self.state.draw(&vertices, &indices);
   }

   fn text(
      &mut self,
      rect: Rect,
      font: &Self::Font,
      text: &str,
      color: Color,
      alignment: Alignment,
   ) -> f32 {
      0.0
   }
}

impl RenderBackend for OpenGlBackend {
   type Image = Image;

   type Framebuffer = Framebuffer;

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      Framebuffer {}
   }

   fn draw_to(&mut self, framebuffer: &Framebuffer, f: impl FnOnce(&mut Self)) {}

   fn clear(&mut self, color: Color) {
      let (r, g, b, a) = normalized_color(color);
      unsafe {
         self.gl.clear_color(r, g, b, a);
         self.gl.clear(glow::COLOR_BUFFER_BIT);
      }
   }

   fn image(&mut self, mut position: Point, image: &Image) {
      position += self.state.transform().translation;
      let (fwidth, fheight) = (image.width() as f32, image.height() as f32);
      let color = image.color.unwrap_or(Color::WHITE);
      let vertices = [
         Vertex::textured_colored(position, point(0.0, 0.0), color),
         Vertex::textured_colored(position + vector(fwidth, 0.0), point(1.0, 0.0), color),
         Vertex::textured_colored(position + vector(fwidth, fheight), point(1.0, 1.0), color),
         Vertex::textured_colored(position + vector(0.0, fwidth), point(0.0, 1.0), color),
      ];
      let indices = [0, 1, 2, 2, 3, 0];
      let texture = image.upload(&self.gl);
      unsafe {
         self.gl.active_texture(glow::TEXTURE0);
         self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         let swizzle_mask = if image.color.is_some() {
            [glow::ONE, glow::ONE, glow::ONE, glow::ALPHA]
         } else {
            [glow::RED, glow::GREEN, glow::BLUE, glow::ALPHA]
         };
         self.gl.texture_swizzle_mask(glow::TEXTURE_2D, &swizzle_mask);
         self.state.draw(&vertices, &indices);
         self.state.bind_null_texture();
      }
   }

   fn framebuffer(&mut self, position: Point, framebuffer: &Framebuffer) {}

   fn scale(&mut self, scale: Vector) {}

   fn set_blend_mode(&mut self, new_blend_mode: netcanv_renderer::BlendMode) {}
}
