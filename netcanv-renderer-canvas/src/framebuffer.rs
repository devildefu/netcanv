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
