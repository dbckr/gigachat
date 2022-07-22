/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{fs::{DirBuilder, OpenOptions, File}, io::{Write, Read}, path::PathBuf};

use curl::easy::Easy;
use egui::{TextureHandle, ColorImage};
use image::{DynamicImage};
use itertools::Itertools;
use glob::glob;
use tracing::{info, warn};
use crate::error_util::{LogErrResult, LogErrOption};

use super::CssAnimationData;

pub fn get_image_data(
  url: &str,
  path: PathBuf,
  id: &str,
  extension: &Option<String>,
  easy: &mut Easy,
  css_anim: Option<CssAnimationData>
) -> Option<Vec<(ColorImage, u16)>> {
  let inner =
    || -> std::result::Result<Option<Vec<(ColorImage, u16)>>, anyhow::Error> {
      DirBuilder::new().recursive(true).create(&path)?;

      let paths = match glob(&format!("{}{}.*", &path.to_str().log_expect("path to string failed"), id)) {
        Ok(paths) => paths,
        Err(e) => panic!("{}", e)
      };

      match paths.last() {
        Some(x) => {
          let filepath : std::path::PathBuf = x?.as_path().to_owned();
          let buffer = load_file_into_buffer(filepath.to_str().log_unwrap());
          match filepath.extension() {
            Some(ext) => Ok(load_image(ext.to_str().log_unwrap(), &buffer, css_anim)),
            None => Ok(None)
          }
        }
        None => {
          let mut extension = extension.as_ref().map(|f| f.to_owned());
          let mut success = false;
          let mut buffer: Vec<u8> = Default::default();

          easy.url(url)?;
          let mut transfer = easy.transfer();
          
          transfer.header_function(|data| {
            let result = std::str::from_utf8(data);
            //info!("result {:?}", result);
            if let Ok(header) = result && header.contains("200 OK") {
              success = true;
            }
            if extension.is_none() && let Ok(header) = result && (header.to_lowercase().contains("content-disposition") || header.to_lowercase().contains("content-type")) {
              //TODO: extract extension using regex
              if header.to_lowercase().contains(".png") || header.to_lowercase().trim_end().ends_with("/png") {
                extension = Some("png".to_owned());
              }
              else if header.to_lowercase().contains(".gif") || header.to_lowercase().trim_end().ends_with("/gif") {
                extension = Some("gif".to_owned());
              }
              else if header.to_lowercase().contains(".webp") || header.to_lowercase().trim_end().ends_with("/webp") {
                extension = Some("webp".to_owned());
              }
              else {
                extension = Some("png".to_owned());
              }
            }
            true
          })?;
          transfer.write_function(|data| {
            for byte in data {
              buffer.push(byte.to_owned());
            }
            Ok(data.len())
          })?;
          transfer.perform()?;
          drop(transfer);

          if !success {
            return Ok(None);
          }

          // If 7TV or unknown extension, try loading it as gif and webp to determine format
          // (7TV is completely unreliable for determining format)
          if path.as_os_str().to_str().and_then(|f| Some(f.contains("7tv"))) == Some(true) || extension.is_none() {
            if load_animated_gif(&buffer).is_some() {
              extension = Some("gif".to_owned())
            }
            else if load_animated_webp(&buffer).is_some() {
              extension = Some("webp".to_owned())
            }
            else {
              extension = Some("png".to_owned())
            }
          }

          match extension { 
            Some(ext) => {
              let mut f = OpenOptions::new()
              .create_new(true)
              .write(true)
              .open(path.join(format!("{}.{}", id, ext)))?;

              f.write_all(&buffer)?;
              Ok(load_image(&ext, &buffer, css_anim))
            },
            None => Ok(None)
          }
        } 
      }
    };

  match inner() {
    Ok(x) => x,
    Err(x) => { info!("failed to (down)load emote {} {} {}", id, url, x); None },
  }
}

fn load_image(
  extension: &str,
  buffer: &[u8],
  css_anim: Option<CssAnimationData>
) -> Option<Vec<(ColorImage, u16)>> {
  match extension {
    "png" => match image::load_from_memory(buffer) {
      Ok(img) => {
        match css_anim {
          None => Some([(to_egui_image(img), 0)].to_vec()),
          Some(data) => process_dgg_sprite_png(img, data)
        }
      },
      _ => None,
    },
    "gif" => load_animated_gif(buffer),
    "webp" => load_animated_webp(buffer),
    _ => None
  }
}

fn process_dgg_sprite_png(img: DynamicImage, data: CssAnimationData) -> Option<Vec<(ColorImage, u16)>> {
  let mut frames : Vec<(ColorImage, u16)> = Default::default();
  let mut x_start = 0;
  let frame_time = (data.cycle_time_msec / data.steps) as u16;
  while x_start < img.width() {
    let frame = img.crop_imm(x_start, 0, data.width, img.height());
    frames.push((to_egui_image(frame), frame_time));
    x_start += data.width;
  }
  Some(frames)
}

pub fn load_animated_gif(buffer: &[u8]) -> Option<Vec<(ColorImage, u16)>> {
  let mut loaded_frames: Vec<(ColorImage, u16)> = Default::default();
  let mut gif_opts = gif::DecodeOptions::new();
  gif_opts.set_color_output(gif::ColorOutput::Indexed);

  if let Ok(mut decoder) = gif_opts.read_info(buffer).inspect_err(|f| warn!("{:?}", f)) {
    let mut screen = gif_dispose::Screen::new_decoder(&decoder);

    while let Ok(frame) = decoder.read_next_frame() && let Some(frame) = frame {
      let frametime = match frame.delay {
        x if x <= 1 => 100,
        x => x * 10
      };
      match screen.blit_frame(frame) {
        Ok(_) => {
          let x = screen.pixels.pixels().flat_map(|px| [px.r, px.g, px.b, px.a]).collect_vec();
          let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> =
            image::ImageBuffer::from_raw(screen.pixels.width() as u32, screen.pixels.height() as u32, x);
          let image = DynamicImage::from(imgbufopt.log_unwrap());
          loaded_frames.push((to_egui_image(image), frametime));
        },
        Err(e) => info!("Error processing gif: {}", e)
      }
    }
  }

  if !loaded_frames.is_empty() {
    Some(loaded_frames)
  } else {
    None
  }
}
#[cfg(not(feature = "webp"))]
pub fn load_animated_webp(_: &[u8]) -> Option<Vec<(ColorImage, u16)>> {
  None
}

#[cfg(feature = "webp")]
pub fn load_animated_webp(buffer: &[u8]) -> Option<Vec<(ColorImage, u16)>> {
  let mut loaded_frames: Vec<(ColorImage, u16)> = Default::default();
  if let Ok(decoder) = webp_animation::Decoder::new(buffer).inspect_err(|f| warn!("{:?}", f)) {
  let mut last_timestamp: u16 = 0;
    for frame in decoder.into_iter() {
      let (width, height) = frame.dimensions();
      let frametime = match frame.timestamp() as u16 - last_timestamp {
        x if x <= 10 => 100,
        x => x
      };
      last_timestamp = frame.timestamp() as u16;
      let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, _>> =
        image::ImageBuffer::from_raw(width, height, frame.data().to_vec());
      if let Some(imgbuf) = imgbufopt {
        let handle = to_egui_image(DynamicImage::from(imgbuf));
        loaded_frames.push((handle, frametime));
      } else {
        info!("failed frame load webp");
      }
    }
  }
  if !loaded_frames.is_empty() {
    Some(loaded_frames)
  } else {
    None
  }
}

pub fn load_file_into_buffer (filepath : &str) -> Vec<u8> {
  let mut file = File::open(filepath).log_unwrap();
  let mut buf: Vec<u8> = Default::default();
  file.read_to_end(&mut buf).log_expect("file not found");
  buf
}

pub fn load_to_texture_handles(ctx : &egui::Context, frames : Option<Vec<(ColorImage, u16)>>) -> Option<Vec<(TextureHandle, u16)>> {
  frames.map(|frames| frames.into_iter().map(|(frame, msec)| { (load_image_into_texture_handle(ctx, frame), msec) }).collect())
}

pub fn load_image_into_texture_handle(
  ctx: &egui::Context,
  image: ColorImage,
) -> TextureHandle {
  let uid = rand::random::<u64>(); //TODO: hash the image to create uid
  ctx.load_texture(uid.to_string(), image)
}

pub fn to_egui_image(
  image: image::DynamicImage
) -> ColorImage {
  //let resize_width = (image.width() as f32 * super::super::ui::EMOTE_HEIGHT * 2. / image.height() as f32).floor() as u32;
  //let image = image.resize(resize_width, (super::super::ui::EMOTE_HEIGHT * 2.).floor() as u32, FilterType::Lanczos3);
  let size = [image.width() as usize, image.height() as usize];
  let image_buffer = image.to_rgba8();
  let pixels = image_buffer.as_flat_samples();
  let cimg = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
  cimg
}