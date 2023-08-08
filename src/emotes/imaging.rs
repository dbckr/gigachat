/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{fs::{DirBuilder, OpenOptions, File}, io::{Write, Read}, path::PathBuf, hash::{Hash, Hasher}};

use ahash::AHasher;
use egui::ColorImage;
use egui_extras::RetainedImage;
use image::DynamicImage;
use itertools::Itertools;
use reqwest::{header::{CONTENT_DISPOSITION, CONTENT_TYPE, ACCEPT}, StatusCode};
use tracing::{info, warn, debug};
use tracing_unwrap::{OptionExt, ResultExt};
use usvg::TreeParsing;
use super::CssAnimationData;

pub async fn get_image_data(
  name: &str,
  urls: &[&str],
  path: &PathBuf,
  id: &str,
  extension: &Option<String>,
  client: &reqwest::Client,
  css_anim: &Option<CssAnimationData>
) -> Option<Vec<(ColorImage, u16)>> {
  let expect_path = path.to_str().expect_or_log("path to string failed");
  match DirBuilder::new().recursive(true).create(path) {
    Ok(_) => {},
    Err(x) => {
      warn!("Failed to load emote {name}: could not create directory {expect_path}: {x}");  
      return None;
    }
  };
  for url in urls {
    let image_data = get_image_data_inner(url, expect_path, id, extension, client, css_anim).await;
    match image_data {
      Ok(x) => { return Some(x) },
      Err(x) => debug!("Failed to load emote {} from url {} due to error: {}", name, url, x)
    }
  }

  let buffer : [u8;0] = [];
  //TODO: Add some internal state to manage failed downloads instead of panicking, to prevent infinite download attempt loop
  write_file_to_disk(expect_path, id, &"unknown".to_owned(), &buffer).expect("Failed to store downloaded image");
  let urls_str = urls.join(", ");
  warn!("Failed to load emote {} from any of the provided urls: {urls_str}", name);  
  None
}

async fn get_image_data_inner(
  url: &str,
  expect_path: &str,
  id: &str,
  extension: &Option<String>,
  client: &reqwest::Client,
  css_anim: &Option<CssAnimationData>
) -> Result<Vec<(ColorImage, u16)>, anyhow::Error> {
  /*let path = match glob(&format!("{expect_path}{id}.*")) {
    Ok(paths) => paths,
    Err(e) => panic!("{}", e)
  }.find_map(|p| p.ok().map(|f| f.as_path()));*/
  let possible_paths = [
    format!("{expect_path}{id}.png"),
    format!("{expect_path}{id}.gif"),
    format!("{expect_path}{id}.webp"),
    format!("{expect_path}{id}.svg"),
  ];
  let path = possible_paths.iter().find_map(|p| {
    let path = std::path::Path::new(p);
    if path.exists() {
      Some(path)
    } else {
      None
    }
  });

  let mut ext : Option<&str> = None;
  let mut image_load_failure = true;
  let result = if let Some(x) = path && let Some(e) = x.extension() && let Some(extension) = e.to_str() {
    ext = Some(extension);
    if let Some(buffer) = load_file_into_buffer(x.to_str().unwrap_or_log()) {
      load_image(extension, &buffer, css_anim)
    } else {
      image_load_failure = true;
      Err(anyhow::Error::msg("failed to load file"))
    }
  } else {
    image_load_failure = false;
    Err(anyhow::Error::msg("no existing file found"))
  };

  if result.is_err() && !ext.is_some_and(|x| x == "unknown") {
    let mut extension = extension.as_ref().map(|f| f.to_owned());
    let buffer = download_image(expect_path, id, &mut extension, url, client, image_load_failure).await?;
    load_image(&extension.unwrap(), &buffer, css_anim)
  }
  else {
    result
  }
}

async fn download_image(expect_path: &str, id: &str, extension: &mut Option<String>, url: &str, client: &reqwest::Client, bad_extension: bool) -> Result<Vec<u8>, anyhow::Error> {
  let req = client.get(url);
  let resp = req.send().await?;

  if resp.status() == StatusCode::NOT_FOUND {
    return Err(anyhow::Error::msg("404 Not Found"))
  }
  
  resp.headers().iter().for_each(|(name, value)| {
    if extension.is_none() && let Ok(header) = value.to_str() {
      if name == CONTENT_DISPOSITION || name == CONTENT_TYPE {
        //TODO: extract extension using regex
        if header.to_lowercase().contains(".png") || header.to_lowercase().trim_end().ends_with("/png") {
          *extension = Some("png".to_owned());
        }
        else if header.to_lowercase().contains(".gif") || header.to_lowercase().trim_end().ends_with("/gif") {
          *extension = Some("gif".to_owned());
        }
        else if header.to_lowercase().contains(".webp") || header.to_lowercase().trim_end().ends_with("/webp") {
          *extension = Some("webp".to_owned());
        }
        else if url.ends_with(".svg") { // YT svg emote urls
          *extension = Some("svg".to_owned());
        }
      }
      else if name == ACCEPT {
        if header.to_lowercase().contains("image/png") || header.to_lowercase().contains("image/apng") {
          *extension = Some("png".to_owned());
        }
        else if header.to_lowercase().contains("image/gif") {
          *extension = Some("gif".to_owned());
        }
        else if header.to_lowercase().contains("image/webp") {
          *extension = Some("webp".to_owned());
        }
      }
    }
  });

  let buffer = resp.bytes().await?.to_vec();

  // If 7TV or unknown extension, try loading it as gif and webp to determine format
  // (7TV is completely unreliable for determining format)
  if expect_path.contains("7tv") || extension.is_none() || bad_extension {
    if load_animated_gif(&buffer).is_ok_and(|f| !f.is_empty()) {
      *extension = Some("gif".to_owned())
    }
    else if load_animated_webp(&buffer).is_ok_and(|f| !f.is_empty()) {
      *extension = Some("webp".to_owned())
    }
    else if !bad_extension || image::load_from_memory(&buffer).is_ok() {
      *extension = Some("png".to_owned())
    }
    else {
      *extension = Some("unknown".to_owned())
    }
  }

  if let Some(ext) = extension {
    debug!("saving image downloaded from {url} to disk: {expect_path}{id}.{ext}");
    write_file_to_disk(expect_path, id, ext, &buffer)?;
  }
  else {
    return Err(anyhow::Error::msg("Unable to determine image extension"));
  }

  Ok(buffer)
}

fn write_file_to_disk(expect_path: &str, id: &str, ext: &String, buffer: &[u8]) -> Result<(), anyhow::Error> {
    let mut f = OpenOptions::new()
    .create(true)
    .write(true)
    .open(std::path::Path::new(&format!("{expect_path}{id}.{ext}")))?;
    f.write_all(buffer)?;
    Ok(())
}

fn load_image(
  extension: &str,
  buffer: &[u8],
  css_anim: &Option<CssAnimationData>
) -> Result<Vec<(ColorImage, u16)>, anyhow::Error> {
  match extension {
    "png" => {
      let img = image::load_from_memory(buffer)?;
      match css_anim {
        None => Ok([(to_egui_image(img), 0)].to_vec()),
        Some(data) => process_dgg_sprite_png(img, data)
      }
    },
    "gif" => load_animated_gif(buffer),
    "webp" => load_animated_webp(buffer),
    "svg" => {
      let opt = usvg::Options::default();
      // Get file's absolute directory.
      //opt.resources_dir = std::fs::canonicalize(&args[1]).ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
      //opt.fontdb.load_system_fonts();
      let rtree = usvg::Tree::from_data(buffer, &opt).unwrap();
      let pixmap_size = rtree.size.to_int_size();//.to_screen_size();
      let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
      let tree = resvg::Tree::from_usvg(&rtree);
      tree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());
      let pixels = pixmap.encode_png()?;
      let img = image::load_from_memory(&pixels)?;
      Ok([(to_egui_image(img), 0)].to_vec())
    }
    _ => Err(anyhow::Error::msg("Extension argument must be png, gif, or webp"))
  }
}

fn process_dgg_sprite_png(img: DynamicImage, data: &CssAnimationData) -> Result<Vec<(ColorImage, u16)>, anyhow::Error> {
  let mut frames : Vec<(ColorImage, u16)> = Default::default();
  let mut x_start = 0;

  if data.steps == 1 {
    let frame = img.crop_imm(x_start, 0, data.width, img.height());
    frames.push((to_egui_image(frame), data.cycle_time_msec as u16));
  }
  else {
    let frame_time = (data.cycle_time_msec / data.steps) as u16;
    let frame_width = if img.width() % data.width == 0 {
      data.width
    } else {
      img.width() / data.steps as u32
    };
    while x_start < img.width() {
      if x_start + frame_width <= img.width() {
        let frame = img.crop_imm(x_start, 0, frame_width, img.height());
        frames.push((to_egui_image(frame), frame_time));
      }
      x_start += frame_width;
    }
  }
  Ok(frames)
}

pub fn load_animated_gif(buffer: &[u8]) -> Result<Vec<(ColorImage, u16)>, anyhow::Error> {
  let mut loaded_frames: Vec<(ColorImage, u16)> = Default::default();
  let mut gif_opts = gif::DecodeOptions::new();
  gif_opts.set_color_output(gif::ColorOutput::Indexed);

  let mut decoder = gif_opts.read_info(buffer)?;
  let mut screen = gif_dispose::Screen::new_decoder(&decoder);

  while let Ok(frame) = decoder.read_next_frame() && let Some(frame) = frame {
    let frametime = match frame.delay {
      x if x <= 1 => 100,
      x => x * 10
    };
    screen.blit_frame(frame)?;
    let x = screen.pixels.pixels().flat_map(|px| [px.r, px.g, px.b, px.a]).collect_vec();
    let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> =
      image::ImageBuffer::from_raw(screen.pixels.width() as u32, screen.pixels.height() as u32, x);
    let image = DynamicImage::from(imgbufopt.unwrap_or_log());
    loaded_frames.push((to_egui_image(image), frametime));
  }
  Ok(loaded_frames)
}

pub fn load_animated_webp(buffer: &[u8]) -> Result<Vec<(ColorImage, u16)>, anyhow::Error> {
  let mut loaded_frames: Vec<(ColorImage, u16)> = Default::default();
  let decoder = match webp_animation::Decoder::new(buffer) {
    Ok(x) => x,
    Err(e) => { return Err(anyhow::Error::msg(format!("{e:?}"))) }
  };
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
  Ok(loaded_frames)
}

pub fn load_file_into_buffer (filepath : &str) -> Option<Vec<u8>> {
  if let Ok(mut file) = File::open(filepath) {
    let mut buf: Vec<u8> = Default::default();
    file.read_to_end(&mut buf).expect_or_log("file not found");
    Some(buf)
  } else {
    None
  }
}

pub fn load_to_texture_handles(/*ctx : &egui::Context,*/ frames : Option<Vec<(ColorImage, u16)>>) -> Option<Vec<(RetainedImage, u16)>> {
  frames.map(|frames| frames.into_iter().map(|(frame, msec)| { (load_image_into_texture_handle(frame), msec) }).collect())
}

pub fn load_image_into_texture_handle(
  //ctx: &egui::Context,
  image: ColorImage,
) -> RetainedImage {
  let mut s = AHasher::default();
  image.pixels.hash(&mut s);
  let uid = s.finish();
  //ctx.load_texture(uid.to_string(), image, egui::TextureOptions::LINEAR)
  RetainedImage::from_color_image(format!("{uid}"), image)
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