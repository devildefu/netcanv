use web_sys::HtmlImageElement;

#[derive(Clone)]
pub struct Image {
   width: u32,
   height: u32,
   data: Vec<u8>,
}

impl Image {
   pub fn build(&self) -> HtmlImageElement {
      use image::png::PngEncoder;

      let mut data: Vec<u8> = vec![];
      let encoder = PngEncoder::new(&mut data);

      // Encode pixel data to png, so we can use encode it to base64 later
      encoder.encode(&self.data, self.width, self.height, image::ColorType::Rgba8);

      let image = HtmlImageElement::new_with_width_and_height(self.width, self.height).unwrap();

      // Encode png to base64 and set image's src to it
      // Browsers are weird I think
      let base64 = format!("data:image/png;base64,{}", base64::encode(&data));
      image.set_src(&base64);

      image
   }

   /// Get a reference to the image's data.
   pub fn data(&self) -> &Vec<u8> {
      self.data.as_ref()
   }
}

impl netcanv_renderer::Image for Image {
   fn from_rgba(width: u32, height: u32, pixel_data: &[u8]) -> Self {
      Self {
         width,
         height,
         data: pixel_data.to_vec(),
      }
   }

   fn colorized(&self, color: netcanv_renderer::paws::Color) -> Self {
      let mut data = self.data.clone();

      for pixel in data.chunks_mut(4) {
         pixel[0] = color.r;
         pixel[1] = color.g;
         pixel[2] = color.b;
         pixel[3] = ((pixel[3] as f32 / 255.0) * (color.a as f32 / 255.0) * 255.0) as u8;
      }

      Self {
         width: self.width,
         height: self.height,
         data,
      }
   }

   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }
}
