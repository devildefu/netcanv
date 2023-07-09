use std::io::Cursor;

use image::codecs::png::{PngDecoder, PngEncoder};
use image::codecs::webp::WebPDecoder;
use image::{ColorType, ImageDecoder, ImageEncoder, Rgba, RgbaImage};

use crate::paint_canvas::cache_layer::CachedChunk;
use crate::paint_canvas::chunk::Chunk;
use crate::Error;

pub struct ImageCoder;

impl ImageCoder {
   /// The maximum size threshold for a PNG to get converted to lossy WebP before network
   /// transmission.
   const MAX_PNG_SIZE: usize = 32 * 1024;
   /// The quality of encoded WebP files.
   // Note to self in the future: the libwebp quality factor ranges from 0.0 to 100.0, not
   // from 0.0 to 1.0.
   // 80% is a fairly sane default that preserves most of the image's quality while still retaining a
   // good compression ratio.
   const WEBP_QUALITY: f32 = 80.0;

   /// Encodes an image to PNG data asynchronously.
   pub fn encode_png_data(image: RgbaImage) -> netcanv::Result<Vec<u8>> {
      let mut bytes: Vec<u8> = Vec::new();
      match PngEncoder::new(Cursor::new(&mut bytes)).write_image(
         &image,
         image.width(),
         image.height(),
         ColorType::Rgba8,
      ) {
         Ok(()) => (),
         Err(error) => {
            log::error!("error while encoding: {}", error);
            return Err(error.into());
         }
      }
      Ok(bytes)
   }

   /// Encodes an image to WebP asynchronously.
   fn encode_webp_data(image: RgbaImage) -> netcanv::Result<Vec<u8>> {
      todo!()
      // Ok(tokio::task::spawn_blocking(move || {
      //    let image = DynamicImage::ImageRgba8(image);
      //    let encoder = webp::Encoder::from_image(&image).unwrap();
      //    encoder.encode(Self::WEBP_QUALITY).to_owned()
      // })
      // .await?)
   }

   /// Encodes a network image asynchronously. This encodes PNG, as well as WebP if the PNG is too
   /// large, and returns both images.
   pub fn encode_network_data(image: RgbaImage) -> netcanv::Result<CachedChunk> {
      let png = Self::encode_png_data(image.clone())?;
      let webp = if png.len() > Self::MAX_PNG_SIZE {
         Some(Self::encode_webp_data(image)?)
      } else {
         None
      };
      Ok(CachedChunk { png, webp })
   }

   /// Decodes a PNG file into the given sub-chunk.
   pub fn decode_png_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      let decoder = PngDecoder::new(Cursor::new(data))?;
      if decoder.color_type() != ColorType::Rgba8 {
         log::warn!("received non-RGBA image data, ignoring");
         return Err(Error::NonRgbaChunkImage);
      }
      let mut image = RgbaImage::from_pixel(Chunk::SIZE.0, Chunk::SIZE.1, Rgba([0, 0, 0, 0]));
      decoder.read_image(&mut image)?;
      Ok(image)
   }

   /// Decodes a WebP file into the given sub-chunk.
   fn decode_webp_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      let decoder = WebPDecoder::new(Cursor::new(data))?;
      if decoder.color_type() != ColorType::Rgba8 {
         log::warn!("received non-RGBA image data, ignoring");
         return Err(Error::NonRgbaChunkImage);
      }
      let mut image = RgbaImage::from_pixel(Chunk::SIZE.0, Chunk::SIZE.1, Rgba([0, 0, 0, 0]));
      decoder.read_image(&mut image)?;
      Ok(image)
   }

   /// Decodes a PNG or WebP file into the given sub-chunk, depending on what's actually stored in
   /// `data`.
   pub fn decode_network_data(data: &[u8]) -> netcanv::Result<RgbaImage> {
      // Try WebP first.
      let image = Self::decode_webp_data(data).or_else(|_| Self::decode_png_data(data))?;
      if image.dimensions() != Chunk::SIZE {
         log::error!(
            "received chunk with invalid size. got: {:?}, expected {:?}",
            image.dimensions(),
            Chunk::SIZE
         );
         Err(Error::InvalidChunkImageSize)
      } else {
         Ok(image)
      }
   }
}

impl Drop for ImageCoder {
   fn drop(&mut self) {
      // self.runtime.block_on(async {
      //    let (channel, join_handle) = self.decoder_quitter.take().unwrap();
      //    let _ = channel.send(());
      //    let _ = join_handle.await;
      // });
   }
}
