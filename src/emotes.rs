use curl::easy::Easy;
use eframe::{
  egui::{self},
  epaint::{ColorImage, TextureHandle},
};
use failure;
use gif::DisposalMethod;
use glob::glob;
use image::imageops::FilterType;
use image::DynamicImage;
use serde_json::Value;
use std::{collections::HashMap};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::str;

//#[derive(Clone)]
pub struct Emote {
  pub name: String,
  pub id: String,
  pub data: Option<Vec<(TextureHandle, u16)>>,
  pub loaded: bool,
  pub duration_msec: u16,
  url: String,
  path: String,
  extension: Option<String>,
}

pub struct EmoteLoader {
  easy: Easy,
}

impl Default for EmoteLoader {
  fn default() -> Self {
    Self { easy: Easy::new() }
  }
}

impl EmoteLoader {
  pub fn load_channel_emotes(
    &mut self,
    channel_id: &String,
  ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let Self { easy: _ } = self;
    let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
    let ffz_emotes = self.process_emote_json(
      &ffz_url,
      &format!("generated/ffz-channel-json-{}", channel_id),
    )?;
    let bttv_url = format!(
      "https://api.betterttv.net/3/cached/users/twitch/{}",
      channel_id
    );
    let bttv_emotes = self.process_emote_json(
      &bttv_url,
      &format!("generated/bttv-channel-json-{}", channel_id),
    )?;
    let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
    let seventv_emotes = self.process_emote_json(
      &seventv_url,
      &format!("generated/7tv-channel-json-{}", channel_id),
    )?;

    let mut result: HashMap<String, Emote> = HashMap::new();
    for emote in ffz_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    for emote in bttv_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    for emote in seventv_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    Ok(result)
  }

  pub fn load_global_emotes(
    &mut self,
  ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let bttv_emotes = self.process_emote_json(
      "https://api.betterttv.net/3/cached/emotes/global",
      "generated/bttv-global-json",
    )?;
    let seventv_emotes = self.process_emote_json(
      "https://api.7tv.app/v2/emotes/global",
      "generated/7tv-global-json",
    )?;

    let mut result: HashMap<String, Emote> = HashMap::new();

    for emote in bttv_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    for emote in seventv_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    Ok(result)

    //process_twitch_json("config/twitch-json-data")?;
    //process_emote_list("config/twitch-global")?;
    //process_emote_list("config/generic-emoji-list")?;
    //process_emote_list("config/emoji-unicode")?;
  }

  pub fn load_list_file(filename: &str) -> std::result::Result<Vec<String>, failure::Error> {
    let file = File::open(filename).expect("no such file");
    Ok(BufReader::new(file)
      .lines()
      .map(|l| l.expect("Could not parse line"))
      .collect())
  }

  pub fn process_emote_list(filename: &str) -> std::result::Result<(), failure::Error> {
    println!("processing emote list {}", filename);

    let mut f = OpenOptions::new()
      .append(true)
      .create(true) // Optionally create the file if it doesn't already exist
      .open("generated/emotes")
      .expect("Unable to open file");

    let file = File::open(filename).expect("no such file");
    for l in BufReader::new(file).lines() {
      let line = l.expect("Could not parse line");
      writeln!(f, "{}", line)?;
    }

    Ok(())
  }

  pub fn process_emote_json(
    &mut self,
    url: &str,
    filename: &str,
  ) -> std::result::Result<Vec<Emote>, failure::Error> {
    let Self { easy: _ } = self;

    println!("processing emote json {}", filename);
    let data = self.get_emote_json(url, filename)?;
    let mut v: Value = serde_json::from_str(&data)?;

    let mut emotes: Vec<Emote> = Vec::default();

    if v["channelEmotes"].is_null() == false {
      // BTTV cache api
      for i in v["channelEmotes"].as_array_mut().unwrap() {
        let name = i["code"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let ext = i["imageType"].to_string().trim_matches('"').to_owned();
        let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
        emotes.push(self.get_emote(
          name,
          id,
          imgurl,
          "generated/bttv/".to_owned(),
          Some(ext),
        ));
      }
      for i in v["sharedEmotes"].as_array_mut().unwrap() {
        let name = i["code"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let ext = i["imageType"].to_string().trim_matches('"').to_owned();
        let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
        emotes.push(self.get_emote(
          name,
          id,
          imgurl,
          "generated/bttv/".to_owned(),
          Some(ext),
        ));
      }
    } else if v["emotes"].is_null() == false {
      // BTTV channel name based API
      //e.g. get_emote_json("https://api.betterttv.net/2/channels/jormh", "bttv-jormh-json")?;
      for i in v["emotes"].as_array_mut().unwrap() {
        //emotes.push(Emote { name: i["code"].to_string().trim_matches('"').to_owned(), data: None })
      }
    } else if v["room"].is_null() == false {
      // FFZ
      let setid = v["room"]["set"].to_string();
      for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap() {
        //TODO: Try to get i["urls"]["4"] then i["urls"]["2"] then i["urls"]["1"] in that order of precedence
        let imgurl = format!("https:{}", i["urls"]["4"].to_string().trim_matches('"'));
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        emotes.push(self.get_emote(name, id, imgurl, "generated/ffz/".to_owned(), None));
      }
    } else if v[0].is_null() == false {
      for i in v.as_array_mut().unwrap() {
        if i["code"].is_null() == false {
          let name = i["code"].to_string().trim_matches('"').to_owned();
          let id = i["id"].to_string().trim_matches('"').to_owned();
          let ext = i["imageType"].to_string().trim_matches('"').to_owned();
          let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
          emotes.push(self.get_emote(
            name,
            id,
            imgurl,
            "generated/bttv/".to_owned(),
            Some(ext),
          ));
        } else if i["name"].is_null() == false {
          // 7tv
          //emotes.push(Emote { name: i["name"].to_string().trim_matches('"').to_owned(), data: None })
          let name = i["name"].to_string().trim_matches('"').to_owned();
          let id = i["id"].to_string().trim_matches('"').to_owned();
          let extension = i["mime"]
            .to_string()
            .trim_matches('"')
            .replace("image/", "");
          let x = i["urls"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_array()
            .unwrap()
            .last()
            .unwrap();
          let imgurl = x.as_str().unwrap();
          emotes.push(self.get_emote(
            name,
            id,
            imgurl.trim_matches('"').to_owned(),
            "generated/7tv/".to_owned(),
            Some(extension),
          ));
        }
      }
    }

    Ok(emotes)
  }

  pub fn process_twitch_json(
    &mut self,
    url: &str,
    filename: &str,
  ) -> std::result::Result<Vec<Emote>, failure::Error> {
    let data = self.get_emote_json(url, filename)?;
    let mut v: Value = serde_json::from_str(&data)?;

    let mut emotes: Vec<Emote> = Vec::default();

    //if v[0]["data"]["channel"]["self"]["availableEmoteSets"].is_null() == false {
    for i in v[0]["data"]["channel"]["self"]["availableEmoteSets"]
      .as_array_mut()
      .unwrap()
    {
      for j in i["emotes"].as_array_mut().unwrap() {
        //writeln!(f, "{}", j["token"].to_string().trim_matches('"'))?;
        //emotes.push(get_emote( j["token"].to_string().trim_matches('"').to_owned(), data: None })
      }
    }
    //}

    Ok(emotes)
  }

  pub fn get_emote_json(
    &mut self,
    url: &str,
    filename: &str,
  ) -> std::result::Result<String, failure::Error> {
    if Path::new(filename).exists() == false {
      let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(filename)
        .expect("Unable to open file");

      let mut easy = Easy::new();
      easy.url(url)?;
      let mut transfer = easy.transfer();
      transfer.write_function(|data| {
        f.write_all(data).expect("Failed to write to file");
        Ok(data.len())
      })?;
      transfer.perform()?;
    }

    let file = File::open(filename).expect("no such file");
    let mut result = String::new();
    for line in BufReader::new(file)
      .lines()
      .filter_map(|result| result.ok())
    {
      result.push_str(&line);
    }
    Ok(result)
  }

  pub fn get_emote(
    &mut self,
    name: String,
    id: String,
    url: String,
    path: String,
    extension: Option<String>,
  ) -> Emote {
    Emote {
      name: name,
      id: id,
      data: None,
      loaded: false,
      url: url,
      path: path,
      extension,
      duration_msec: 0,
    }
  }

  pub fn load_image(&mut self, ctx: &egui::Context, emote: &mut Emote) {
    emote.data = self.get_image_data(ctx, &emote.url, &emote.path, &emote.id, &emote.extension);
    emote.duration_msec = match emote.data.as_ref() {
      Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
      _ => 0,
    };
  }

  fn get_image_data(
    &mut self,
    ctx: &egui::Context,
    url: &str,
    path: &str,
    id: &str,
    extension: &Option<String>,
  ) -> Option<Vec<(TextureHandle, u16)>> {
    let Self { easy } = self;

    //let filename = format!("{}.{}", id, extension);

    let mut inner =
      || -> std::result::Result<Option<Vec<(TextureHandle, u16)>>, failure::Error> {
        if path.len() > 0 {
          DirBuilder::new().recursive(true).create(path)?;
        }

        match glob(&format!("{}{}.*", path, id))?.last() {
          Some(x) => {
            let filepath : std::path::PathBuf = x?.as_path().to_owned();
            let buffer = load_file_into_buffer(filepath.to_str().unwrap());
            match filepath.extension() {
              Some(ext) => Ok(load_image(ctx, ext.to_str().unwrap(), &buffer)),
              None => Ok(None)
            }
          }
          None => {
            if let Some(ref ext) = extension {
              let mut extension: Option<String> = Some(ext.to_owned());
              let mut buffer: Vec<u8> = Default::default();

              easy.url(url)?;
              let mut transfer = easy.transfer();
              if extension == None {
                transfer.header_function(|data| {
                  let result = str::from_utf8(data);
                    if let Ok(header) = result && (header.contains("content-disposition") || header.contains("content-type")) {
                    //TODO: extract extension using regex
                    if header.to_lowercase().contains(".png") || header.to_lowercase().ends_with("png") {
                      extension = Some("png".to_owned());
                    }
                    else if header.to_lowercase().contains(".gif") || header.to_lowercase().ends_with("gif") {
                      extension = Some("gif".to_owned());
                    }
                    else if header.to_lowercase().contains(".webp") || header.to_lowercase().ends_with("webp") {
                      extension = Some("webp".to_owned());
                    }
                  }
                  true
                })?;
              }
              transfer.write_function(|data| {
                //f.write_all(data).expect("Failed to write to file");
                for byte in data {
                  buffer.push(byte.to_owned());
                }
                Ok(data.len())
              })?;
              transfer.perform()?;
              drop(transfer);

              let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(format!("{}{}.{}", path, id, ext))?;

              f.write(&buffer)?;
              Ok(load_image(ctx, &ext, &buffer))
            } else {
              Ok(None)
            }
          }
        }
      };

    match inner() {
      Ok(x) => x,
      Err(x) => None,
    }
  }
}

fn load_image(
  ctx: &egui::Context,
  extension: &str,
  buffer: &[u8],
) -> Option<Vec<(TextureHandle, u16)>> {
  match extension {
    "png" => match image::load_from_memory(&buffer) {
      Ok(img) => Some([(load_image_into_texture_handle(ctx, &resize_image(img)), 0)].to_vec()),
      _ => None,
    },
    "gif" => match load_animated_gif(&buffer) { Some(x) => Some(load_to_texture_handles(ctx, x)), _ => None },
    "webp" => match load_animated_webp(&buffer) { Some(x) => Some(load_to_texture_handles(ctx, x)), _ => None },
    _ => None,
  }
}

pub fn load_animated_gif_is_partial(buffer: &[u8]) -> bool {
  let mut decoder = gif::DecodeOptions::new();
  decoder.set_color_output(gif::ColorOutput::RGBA);
  let mut decoder = decoder.read_info(buffer).unwrap();
  let mut is_partial = false;
  let mut last_transparent: Option<u8> = None;

  while let Some(frame) = decoder.read_next_frame().unwrap() {
    if frame.top > 0 || frame.left > 0 {
      is_partial = true;
      break;
    }
  }
  is_partial
}

pub fn load_animated_gif(buffer: &[u8]) -> Option<Vec<(DynamicImage, u16)>> {

  //let is_partial = load_animated_gif_is_partial(buffer);

  let mut loaded_frames: Vec<(DynamicImage, u16)> = Default::default();
  let mut decoder = gif::DecodeOptions::new();
  decoder.set_color_output(gif::ColorOutput::RGBA);
  let mut decoder = decoder.read_info(buffer).unwrap();
  let mut last_key_img: Option<DynamicImage> = None;
  let mut last_frame_img: Option<DynamicImage> = None;
  let dimensions = (decoder.width(), decoder.height());
  //let reusable_img = &mut last_frameimg;

  while let Some(frame) = decoder.read_next_frame().unwrap() {
    let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> =
      image::ImageBuffer::from_raw(frame.width.into(), frame.height.into(), frame.buffer.to_vec());
      if let Some(imgbuf) = imgbufopt {
        let frametime = match frame.delay {
          x if x <= 1 => 100,
          x => x * 10
        };
        let image = match dimensions {
          (w, h) if frame.width == w && frame.height == h => {
            DynamicImage::from(imgbuf)
          },
          _ => {
            let mut img = DynamicImage::from(image::ImageBuffer::from_pixel(dimensions.0 as u32, dimensions.1 as u32, image::Rgba::<u8>([0, 0, 0, 0]) ));
            image::imageops::replace(&mut img, &DynamicImage::from(imgbuf), frame.left as i64, frame.top as i64);
            img
          }
        }; 
        let handle: DynamicImage;
        println!("{:?} {:?} {:?} {:?} {:?} {:?}", frame.dispose, frame.left, frame.top, frame.width, frame.height, frametime);
        match frame.dispose {
          DisposalMethod::Previous | DisposalMethod::Keep if last_key_img.is_some() => {
            if let Some(last_img) = last_key_img.as_mut() {
              let mut new_img = last_img.clone();
              image::imageops::overlay(&mut new_img, &image, frame.left as i64, frame.top as i64);
              if frame.dispose == DisposalMethod::Keep {
                last_key_img = Some(new_img.clone());
              }
              last_frame_img = Some(new_img.clone());
              handle = resize_image(new_img);
              loaded_frames.push((handle, frametime));
            } else {
              println!("failed partial frame load gif");
            }
          },
          DisposalMethod::Background if last_frame_img.is_some() => {
            if let Some(last_img) = last_frame_img.as_mut() {
              let mut new_img = last_img.clone();
              image::imageops::overlay(&mut new_img, &image, frame.left as i64, frame.top as i64);
              last_frame_img = None;// Some(DynamicImage::from(image::ImageBuffer::from_pixel(dimensions.0 as u32, dimensions.1 as u32, image::Rgba::<u8>([0, 0, 0, 0]) )));
              last_key_img = None;
              handle = resize_image(new_img);
              loaded_frames.push((handle, frametime));
            } else {
              println!("failed partial frame load gif");
            }
          },
          _ => {
            match frame.dispose {
              DisposalMethod::Keep => {
                last_key_img = Some(image.clone());
                last_frame_img = Some(image.clone());
              },
              DisposalMethod::Background => {
                last_frame_img = None;
                last_key_img = None;
              },
              _ => last_frame_img = Some(image.clone())
            };
            handle = resize_image(image);
            loaded_frames.push((handle, frametime));
            //println!("success: full frame");
          }
        };   
    }
    else {
      println!("failed frame load gif");
    }
  }

  if loaded_frames.len() > 0 {
    Some(loaded_frames)
  } else {
    None
  }
}

pub fn load_animated_webp(buffer: &[u8]) -> Option<Vec<(DynamicImage, u16)>> {
  let mut loaded_frames: Vec<(DynamicImage, u16)> = Default::default();
  let decoder = webp_animation::Decoder::new(&buffer).unwrap();
  let mut last_timestamp: u16 = 0;
  for frame in decoder.into_iter() {
    let (width, height) = frame.dimensions();
    let frametime = match (frame.timestamp() as u16 - last_timestamp) {
      x if x <= 10 => 100,
      x => x
    };
    last_timestamp = frame.timestamp() as u16;
    //println!("{:?} {:?} {}", frame.dimensions(), frame.color_mode(), frame.timestamp());
    let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, _>> =
      image::ImageBuffer::from_raw(width, height, frame.data().to_vec());
    if let Some(imgbuf) = imgbufopt {
      let handle = resize_image(DynamicImage::from(imgbuf));
      loaded_frames.push((handle, frametime));
    } else {
      println!("failed frame load webp");
    }
  }
  if loaded_frames.len() > 0 {
    Some(loaded_frames)
  } else {
    None
  }
}

pub fn load_file_into_buffer (filepath : &str) -> Vec<u8> {
  let mut file = File::open(filepath).unwrap();
  let mut buf: Vec<u8> = Default::default();
  file.read_to_end(&mut buf).expect("file not found");
  buf
}

fn resize_image(
  image: image::DynamicImage
) -> DynamicImage {
  //let resize_width = image.width() * (24 / image.height());
  //let image = image.resize(resize_width, 24, FilterType::Lanczos3);
  //image.resize(42, 42, FilterType::Nearest)
  image
}

fn load_to_texture_handles(ctx : &egui::Context, frames : Vec<(DynamicImage, u16)>) -> Vec<(TextureHandle, u16)> {
  frames.into_iter().map(|(frame, msec)| { (load_image_into_texture_handle(ctx, &frame), msec) }).collect()
}

fn load_image_into_texture_handle(
  ctx: &egui::Context,
  image: &image::DynamicImage,
) -> TextureHandle {
  let uid = rand::random::<u128>(); //TODO: hash the image to create uid
  let size = [image.width() as _, image.height() as _];
  let image_buffer = image.to_rgba8();
  let pixels = image_buffer.as_flat_samples();
  let cimg = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
  ctx.load_texture(uid.to_string(), cimg)
}