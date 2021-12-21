use netcanv_renderer::paws::{self, Ui};
use std::collections::HashMap;
use std::rc::Rc;
use web_sys::HtmlImageElement;
use winit::event_loop::EventLoop;
use winit::platform::web::WindowExtWebSys;
use winit::window::{Window, WindowBuilder};

mod common;
mod font;
mod framebuffer;
mod image;
mod rendering;
mod state;

pub use crate::common::webp;
pub use crate::font::*;
pub use crate::framebuffer::*;
pub use crate::image::*;
pub use crate::rendering::*;
use crate::state::*;
pub use winit;

pub struct CanvasBackend {
   context: Rc<web_sys::CanvasRenderingContext2d>,
   window: winit::window::Window,
   cache: HashMap<Vec<u8>, HtmlImageElement>,
   states: Vec<State>,
   current_state: usize,
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

      let states = vec![Default::default()];

      Ok(Self {
         context: Rc::new(context),
         window: winit_window,
         cache: HashMap::new(),
         states,
         current_state: 0,
      })
   }

   pub fn window(&self) -> &Window {
      &self.window
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
