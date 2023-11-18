//! The `Save to file` action.


use instant::{Duration, Instant};
use wasm_bindgen::prelude::*;

use crate::assets::Assets;
use crate::backend::{Backend, Image};
use crate::image_coder::ImageCoder;

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
         paint_canvas,
         project_file,
         ..
      }: ActionArgs,
   ) -> netcanv::Result<()> {
      let image = project_file.save_as_png(paint_canvas)?;
      let buf = ImageCoder::encode_png_data(image)?;

      let blob = gloo_file::Blob::new_with_options(buf.as_slice(), Some("image/png"));
      let url = web_sys::Url::create_object_url_with_blob(&blob.into()).unwrap();

      let document = web_sys::window().unwrap().document().unwrap();
      let anchor = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();
      anchor.set_attribute("href", &url).unwrap();
      anchor.set_attribute("download", "canvas.png").unwrap();
      anchor.click();

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
