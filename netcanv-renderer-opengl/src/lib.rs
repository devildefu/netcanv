mod common;
mod font;
mod framebuffer;
mod image;
mod rendering;

use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use glutin::dpi::PhysicalSize;
#[cfg(not(target_arch = "wasm32"))]
use glutin::{Api, ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};

use netcanv_renderer::paws::{point, Ui};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use crate::{font::Font, framebuffer::Framebuffer, image::Image};
use rendering::RenderState;

pub struct OpenGlBackend {
   #[cfg(not(target_arch = "wasm32"))]
   context: WindowedContext<PossiblyCurrent>,
   #[cfg(not(target_arch = "wasm32"))]
   context_size: PhysicalSize<u32>,
   #[cfg(target_arch = "wasm32")]
   window: Window,
   pub(crate) gl: Rc<glow::Context>,
   state: RenderState,
}

impl OpenGlBackend {
   /// Creates a new OpenGL renderer.
   #[cfg(not(target_arch = "wasm32"))]
   pub fn new(window_builder: WindowBuilder, event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
      let context = ContextBuilder::new()
         .with_gl(GlRequest::Specific(Api::OpenGlEs, (3, 0)))
         .with_gl_profile(GlProfile::Core)
         .with_vsync(true)
         .with_multisampling(8)
         .build_windowed(window_builder, event_loop)?;
      let context = unsafe { context.make_current().unwrap() };
      let gl = unsafe {
         glow::Context::from_loader_function(|name| context.get_proc_address(name) as *const _)
      };
      let gl = Rc::new(gl);
      Ok(Self {
         context_size: context.window().inner_size(),
         context,
         state: RenderState::new(Rc::clone(&gl)),
         gl,
      })
   }

   #[cfg(target_arch = "wasm32")]
   pub fn new(window_builder: WindowBuilder, event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
      use wasm_bindgen::JsCast;
      use winit::platform::web::WindowExtWebSys;

      let winit_window = window_builder.build(&event_loop)?;
      let canvas = winit_window.canvas();
      let window = web_sys::window().unwrap();
      let document = window.document().unwrap();
      let body = document.body().unwrap();

      body.append_child(&canvas).expect("Append canvas to HTML body");

      let webgl2_context = canvas
         .get_context("webgl2")
         .unwrap()
         .unwrap()
         .dyn_into::<web_sys::WebGl2RenderingContext>()
         .unwrap();
      let gl = unsafe { glow::Context::from_webgl2_context(webgl2_context) };
      let gl = Rc::new(gl);

      Ok(Self {
         state: RenderState::new(Rc::clone(&gl)),
         window: winit_window,
         gl,
      })
   }

   /// Returns the window.
   #[cfg(not(target_arch = "wasm32"))]
   pub fn window(&self) -> &Window {
      self.context.window()
   }

   #[cfg(target_arch = "wasm32")]
   pub fn window(&self) -> &Window {
      &self.window
   }
}

pub trait UiRenderFrame {
   /// Renders a single frame onto the window.
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<OpenGlBackend> {
   #[cfg(not(target_arch = "wasm32"))]
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      let window_size = self.window().inner_size();
      if self.context_size != window_size {
         self.context.resize(window_size);
      }
      self.state.viewport(window_size.width, window_size.height);
      callback(self);
      self.context.swap_buffers()?;
      Ok(())
   }

   #[cfg(target_arch = "wasm32")]
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      let window_size = self.window().inner_size();
      self.state.viewport(window_size.width, window_size.height);
      callback(self);
      // self.context.swap_buffers()?;
      Ok(())
   }
}
