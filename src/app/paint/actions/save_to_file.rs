//! The `Save to file` action.

use std::io::Cursor;

use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use instant::{Duration, Instant};
use wasm_bindgen::prelude::*;

use crate::assets::Assets;
use crate::backend::{Backend, Image};

use super::{Action, ActionArgs};

pub struct SaveToFileAction {
   icon: Image,
   last_autosave: Instant,
}

impl SaveToFileAction {
   const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icon: Assets::load_svg(renderer, include_bytes!("../../../assets/icons/save.svg")),
         last_autosave: Instant::now(),
      }
   }
}

impl Action for SaveToFileAction {
   fn name(&self) -> &str {
      "save-to-file"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }

   fn perform(
      &mut self,
      ActionArgs {
         assets,
         paint_canvas,
         project_file,
         ..
      }: ActionArgs,
   ) -> netcanv::Result<()> {
      let image = project_file.save_as_png(paint_canvas)?;
      let (width, height) = (image.width(), image.height());

      let mut buf: Vec<u8> = Vec::new();
      let mut cursor = Cursor::new(&mut buf);
      let encoder = PngEncoder::new(&mut cursor);
      encoder.write_image(&image.into_vec(), width, height, ColorType::Rgba8)?;
      show_save_file_picker(buf.as_slice());

      Ok(())
   }

   fn process(
      &mut self,
      ActionArgs {
         paint_canvas,
         project_file,
         ..
      }: ActionArgs,
   ) -> netcanv::Result<()> {
      if project_file.filename().is_some() && self.last_autosave.elapsed() > Self::AUTOSAVE_INTERVAL
      {
         log::info!("autosaving chunks");
         project_file.save(None, paint_canvas)?;
         log::info!("autosave complete");
         self.last_autosave = Instant::now();
      }
      Ok(())
   }
}

#[wasm_bindgen(raw_module = "common")]
extern "C" {
   #[wasm_bindgen(js_name = "showSaveFilePicker")]
   fn show_save_file_picker(buffer: &[u8]);
}
