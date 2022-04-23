/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use curl::easy::Easy;
use eframe::{
  egui::{self, plot::Text},
  epaint::{ColorImage, TextureHandle},
};
use failure;
use glob::glob;
use image::{DynamicImage};
use itertools::Itertools;
use serde_json::Value;
use tokio::{runtime::Runtime, sync::mpsc::{Receiver, Sender}, task::JoinHandle};
use std::{collections::HashMap, net::Shutdown, time::Duration};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::str;

use crate::provider::ProviderName;

pub enum EmoteRequest {
  GlobalEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  ChannelEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String },
  EmoteSetImage { name: String, id : String, url: String, path: String, extension: Option<String>, set_id: String, provider_name: ProviderName },
  TwitchMsgEmoteImage { name: String, id: String },
  Shutdown
}

impl EmoteRequest {
  pub fn new_channel_request(emote: &Emote, channel_name: &str) -> Self {
    EmoteRequest::ChannelEmoteImage { 
      name: emote.name.to_owned(),
      id: emote.id.to_owned(), 
      url: emote.url.to_owned(), 
      path: emote.path.to_owned(), 
      extension: emote.extension.to_owned(), 
      channel_name: channel_name.to_owned()
    }
  } 
  pub fn new_global_request(emote: &Emote) -> Self {
    EmoteRequest::GlobalEmoteImage {
      name: emote.name.to_owned(),
      id: emote.id.to_owned(), 
      url: emote.url.to_owned(), 
      path: emote.path.to_owned(), 
      extension: emote.extension.to_owned()
    }
  }
  pub fn new_emoteset_request(emote: &Emote, provider_name: &ProviderName, set_id: &String) -> Self {
    EmoteRequest::EmoteSetImage {
      name: emote.name.to_owned(),
      id: emote.id.to_owned(), 
      url: emote.url.to_owned(), 
      path: emote.path.to_owned(), 
      extension: emote.extension.to_owned(), 
      provider_name: provider_name.to_owned(),
      set_id: set_id.to_owned()
    }
  }
  pub fn new_twitch_msg_emote_request(emote: &Emote) -> Self {
    EmoteRequest::TwitchMsgEmoteImage { name: emote.name.to_owned(), id: emote.id.to_owned() }
  }
}

pub enum EmoteResponse {
  GlobalEmoteImageLoaded { name : String, data: Option<Vec<(DynamicImage, u16)>> },
  ChannelEmoteImageLoaded { name : String, channel_name: String, data: Option<Vec<(DynamicImage, u16)>> },
  EmoteSetImageLoaded { name: String, set_id: String, provider_name: ProviderName, data: Option<Vec<(DynamicImage, u16)>> },
  TwitchMsgEmoteLoaded { name: String, id: String, data: Option<Vec<(DynamicImage, u16)>> }
}

pub enum EmoteStatus {
  NotLoaded,
  Loading,
  Loaded,
}

//#[derive(Clone)]
pub struct Emote {
  pub name: String,
  pub id: String,
  pub data: Option<Vec<(TextureHandle, u16)>>,
  pub loaded: EmoteStatus,
  pub duration_msec: u16,
  url: String,
  pub path: String,
  pub extension: Option<String>,
}

pub struct EmoteLoader {
  pub tx: Sender<EmoteRequest>,
  pub rx: Receiver<EmoteResponse>,
  handle: JoinHandle<()>,
  pub transparent_img: Option<TextureHandle>
}

impl EmoteLoader {
  pub fn new(runtime: &Runtime) -> Self {
    let (in_tx, mut in_rx) = tokio::sync::mpsc::channel::<EmoteRequest>(32);
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<EmoteResponse>(32);

    let task : JoinHandle<()> = runtime.spawn(async move { 
      let mut easy = Easy::new();
      loop {
        while let Ok(msg) = in_rx.try_recv() {
          let sent_msg = match msg {
            EmoteRequest::ChannelEmoteImage { name, id, url, path, extension, channel_name } => {
              println!("loading channel emote {} for {}", name, channel_name);
              let data = get_image_data(&url, &path, &id, &extension, &mut easy);
              out_tx.try_send(EmoteResponse::ChannelEmoteImageLoaded { name: name, channel_name: channel_name, data: data })
            },
            EmoteRequest::GlobalEmoteImage { name, id, url, path, extension } => {
              println!("loading global emote {}", name);
              let data = get_image_data(&url, &path, &id, &extension, &mut easy);
              out_tx.try_send(EmoteResponse::GlobalEmoteImageLoaded { name: name, data: data })
            },
            EmoteRequest::EmoteSetImage { name, id, url, path, extension, set_id, provider_name } => {
              println!("loading set emote {} for set {}", name, set_id);
              let data = get_image_data(&url, &path, &id, &extension, &mut easy);
              out_tx.try_send(EmoteResponse::EmoteSetImageLoaded { name: name, provider_name: provider_name, set_id: set_id, data: data })
            },
            EmoteRequest::TwitchMsgEmoteImage { name, id } => {
              let data = if let Some(x) = get_image_data(&format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/animated/light/3.0", id), "generated/twitch/", &id, &None, &mut easy) {
                Some(x)
              } else {
                get_image_data(&format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/static/light/3.0", id), "generated/twitch/", &id, &None, &mut easy)
              };
              out_tx.try_send(EmoteResponse::TwitchMsgEmoteLoaded { name: name, id: id, data: data })
            },
            Shutdown => {
              break;
            }
          };
          match sent_msg {
            Ok(()) => (),
            Err(e) => println!("Error sending loaded image event: {}", e)
          };
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
      }
    });

    Self { 
      tx: in_tx,
      rx: out_rx,
      handle: task,
      transparent_img: None
     }
  }

  pub fn close(&mut self) {
    self.handle.abort();
  }
}

impl EmoteLoader {
  pub fn load_channel_emotes(
    &mut self,
    channel_id: &String,
  ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
    let ffz_emotes = self.process_emote_json(
      &ffz_url,
      &format!("generated/ffz-channel-json-{}", channel_id),
      None
    )?;
    let bttv_url = format!(
      "https://api.betterttv.net/3/cached/users/twitch/{}",
      channel_id
    );
    let bttv_emotes = self.process_emote_json(
      &bttv_url,
      &format!("generated/bttv-channel-json-{}", channel_id),
      None
    )?;
    let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
    let seventv_emotes = self.process_emote_json(
      &seventv_url,
      &format!("generated/7tv-channel-json-{}", channel_id),
      None
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
      None
    )?;
    let seventv_emotes = self.process_emote_json(
      "https://api.7tv.app/v2/emotes/global",
      "generated/7tv-global-json",
      None
    )?;

    let mut result: HashMap<String, Emote> = HashMap::new();

    for emote in bttv_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    for emote in seventv_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    Ok(result)
  }

  pub fn twitch_get_emote_set(&mut self, emote_set_id : &String) -> Option<HashMap<String, Emote>> { 
    if emote_set_id.contains(":") || emote_set_id.contains("-") || emote_set_id.contains("emotesv2") {
      return None;
    }

    let emotes = self.process_emote_json(
      &format!("https://api.twitch.tv/helix/chat/emotes/set?emote_set_id={}", emote_set_id),
      &format!("generated/twitch-emote-set-{}", emote_set_id),
      Some([
        ("Authorization", &format!("Bearer {}", crate::provider::twitch::load_token())),
        ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
      ].to_vec())
    );

    match emotes {
      Ok(emotes) => {
        let mut map = HashMap::new();
        for emote in emotes {
          map.insert(emote.name.to_owned(), emote);
        }
        Some(map)
      },
      Err(e) => {
        println!("Error loading emote set: {}", e);
        Some(HashMap::new())
      }
    }
  }

  fn load_list_file(filename: &str) -> std::result::Result<Vec<String>, failure::Error> {
    let file = File::open(filename).expect("no such file");
    Ok(BufReader::new(file)
      .lines()
      .map(|l| l.expect("Could not parse line"))
      .collect())
  }

  fn process_emote_list(filename: &str) -> std::result::Result<(), failure::Error> {
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

  fn process_emote_json(
    &mut self,
    url: &str,
    filename: &str,
    headers: Option<Vec<(&str, &String)>>,
  ) -> std::result::Result<Vec<Emote>, failure::Error> {
    println!("processing emote json {}", filename);
    let data = self.get_emote_json(url, filename, headers)?;
    let mut v: Value = serde_json::from_str(&data)?;
    let mut emotes: Vec<Emote> = Vec::default();
    if v["data"].is_array() { // Twitch Global
      for i in v["data"].as_array_mut().unwrap() {
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let extension;
        let wtf = i["format"].as_array().unwrap();
        let imgurl = if wtf.len() == 2 { //// Disabled -- weezl crate cannot handle twitch animated emotes https://github.com/image-rs/lzw/issues/28
          extension = Some("gif".to_owned());
          i["images"]["url_4x"].to_string().trim_matches('"').replace("/static/", "/animated/").to_owned()
        }
        else {
          extension = Some("png".to_owned());
          i["images"]["url_4x"].to_string().trim_matches('"').to_owned()
        };
        emotes.push(self.get_emote(
          name,
          id,
          imgurl,
          "generated/twitch/".to_owned(),
          extension
        ));
      }
    }
    else if v["channelEmotes"].is_null() == false { // BTTV
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
    } else if v["room"].is_null() == false { // FFZ
      let setid = v["room"]["set"].to_string();
      for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap() {
        //TODO: Try to get i["urls"]["4"] then i["urls"]["2"] then i["urls"]["1"] in that order of precedence
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();

        println!("{} {}", name, id);

        let imgurl = format!("https:{}", i["urls"].as_object_mut().unwrap().values().last().unwrap().to_string().trim_matches('"'));
        emotes.push(self.get_emote(name, id, imgurl, "generated/ffz/".to_owned(), None));
      }
    } else if v[0].is_null() == false {
      for i in v.as_array_mut().unwrap() { // BTTV Global
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
        } else if i["name"].is_null() == false { // 7TV
          let name = i["name"].to_string().trim_matches('"').to_owned();
          let id = i["id"].to_string().trim_matches('"').to_owned();
          let extension = i["mime"].to_string().trim_matches('"').replace("image/", "");
          let x = i["urls"].as_array().unwrap()
            .last().unwrap()
            .as_array().unwrap()
            .last().unwrap();
          let imgurl = x.as_str().unwrap();
          emotes.push(self.get_emote(name, id, imgurl.trim_matches('"').to_owned(), "generated/7tv/".to_owned(), Some(extension)));
        }
      }
    }

    Ok(emotes)
  }

  fn get_emote_json(
    &mut self,
    url: &str,
    filename: &str,
    headers: Option<Vec<(&str, &String)>>,
  ) -> std::result::Result<String, failure::Error> {
    if Path::new(filename).exists() == false {
      let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(filename)
        .expect("Unable to open file");

      let mut easy = Easy::new();
      easy.url(url)?;
      if let Some(headers) = headers {
        let mut list = curl::easy::List::new();
        for head in headers {
          list.append(&format!("{}: {}", head.0, head.1))?;
        }
        easy.http_headers(list)?;
      }
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
      loaded: EmoteStatus::NotLoaded,
      url: url,
      path: path,
      extension,
      duration_msec: 0,
    }
  }
}

fn get_image_data(
  url: &str,
  path: &str,
  id: &str,
  extension: &Option<String>,
  easy: &mut Easy
) -> Option<Vec<(DynamicImage, u16)>> {
  let mut inner =
    || -> std::result::Result<Option<Vec<(DynamicImage, u16)>>, failure::Error> {
      if path.len() > 0 {
        DirBuilder::new().recursive(true).create(path)?;
      }

      let paths = match glob(&format!("{}{}.*", path, id)) {
        Ok(paths) => paths,
        Err(e) => panic!("{}", e)
      };

      match paths.last() {
        Some(x) => {
          let filepath : std::path::PathBuf = x?.as_path().to_owned();
          let buffer = load_file_into_buffer(filepath.to_str().unwrap());
          match filepath.extension() {
            Some(ext) => Ok(load_image(ext.to_str().unwrap(), &buffer)),
            None => Ok(None)
          }
        }
        None => {
          let mut extension = match extension {
            Some(ref ext) => Some(ext.to_owned()),
            None => None
          };
          let mut buffer: Vec<u8> = Default::default();

          easy.url(url)?;
          let mut transfer = easy.transfer();
          if extension.is_none() {
            transfer.header_function(|data| {
              let result = str::from_utf8(data);
              //println!("result {:?}", result);
              if let Ok(header) = result && (header.to_lowercase().contains("content-disposition") || header.to_lowercase().contains("content-type")) {
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

          match extension { 
            Some(ext) => {
              let mut f = OpenOptions::new()
              .create_new(true)
              .write(true)
              .open(format!("{}{}.{}", path, id, ext))?;

            f.write(&buffer)?;
            Ok(load_image(&ext, &buffer))
            },
            None => Ok(None)
          }
        } 
      }
    };

  match inner() {
    Ok(x) => x,
    Err(_) => None,
  }
}

fn load_image(
  extension: &str,
  buffer: &[u8],
) -> Option<Vec<(DynamicImage, u16)>> {
  match extension {
    "png" => match image::load_from_memory(&buffer) {
      Ok(img) => Some([(resize_image(img), 0)].to_vec()),
      _ => None,
    },
    "gif" => match load_animated_gif(&buffer) { Some(x) => Some(x), _ => None },
    "webp" => match load_animated_webp(&buffer) { Some(x) => Some(x), _ => None },
    _ => None,
  }
}

pub fn load_animated_gif(buffer: &[u8]) -> Option<Vec<(DynamicImage, u16)>> {
  let mut loaded_frames: Vec<(DynamicImage, u16)> = Default::default();
  let mut gif_opts = gif::DecodeOptions::new();
  gif_opts.set_color_output(gif::ColorOutput::Indexed);

  let mut decoder = gif_opts.read_info(buffer).unwrap();
  let mut screen = gif_dispose::Screen::new_decoder(&decoder);

  while let Ok(frame) = decoder.read_next_frame() && let Some(frame) = frame {
    let frametime = match frame.delay {
      x if x <= 1 => 100,
      x => x * 10
    };
    match screen.blit_frame(&frame) {
      Ok(_) => {
        let x = screen.pixels.pixels().flat_map(|px| [px.r, px.g, px.b, px.a]).collect_vec();
        let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> =
          image::ImageBuffer::from_raw(screen.pixels.width() as u32, screen.pixels.height() as u32, x);
        let image = DynamicImage::from(imgbufopt.unwrap());
        let handle = resize_image(image);
        loaded_frames.push((handle, frametime));
      },
      Err(e) => println!("Error processing gif: {}", e)
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
    let frametime = match frame.timestamp() as u16 - last_timestamp {
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

pub fn load_to_texture_handles(ctx : &egui::Context, frames : Option<Vec<(DynamicImage, u16)>>) -> Option<Vec<(TextureHandle, u16)>> {
  match frames {
    Some(frames) => Some(frames.into_iter().map(|(frame, msec)| { (load_image_into_texture_handle(ctx, &frame), msec) }).collect()),
    None => None
  }
}

pub fn load_image_into_texture_handle(
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