/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use curl::easy::Easy;
use egui::{epaint::{TextureHandle}};
use egui::ColorImage;

use tokio::{runtime::Runtime, sync::mpsc::{Receiver}, task::JoinHandle};
use std::{collections::HashMap, time::Duration, path::PathBuf};
use std::str;

pub mod fetch;
pub mod imaging;

pub enum EmoteRequest {
  GlobalEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  GlobalBadgeImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  ChannelEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String, css_anim: Option<CssAnimationData> },
  ChannelBadgeImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String },
  TwitchMsgEmoteImage { name: String, id: String }
}

impl EmoteRequest {
  pub fn new_channel_request(emote: &Emote, channel_name: &str) -> Self {
    EmoteRequest::ChannelEmoteImage { 
      name: emote.name.to_owned(),
      id: emote.id.to_owned(), 
      url: emote.url.to_owned(), 
      path: emote.path.to_owned(), 
      extension: emote.extension.to_owned(), 
      channel_name: channel_name.to_owned(),
      css_anim: emote.css_anim.clone()
    }
  } 
  pub fn new_channel_badge_request(emote: &Emote, channel_name: &str) -> Self {
    EmoteRequest::ChannelBadgeImage { 
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
  pub fn new_global_badge_request(emote: &Emote) -> Self {
    EmoteRequest::GlobalBadgeImage {
      name: emote.name.to_owned(),
      id: emote.id.to_owned(), 
      url: emote.url.to_owned(), 
      path: emote.path.to_owned(), 
      extension: emote.extension.to_owned()
    }
  }
  pub fn new_twitch_emote_request(emote: &Emote) -> Self {
    EmoteRequest::TwitchMsgEmoteImage { name: emote.name.to_owned(), id: emote.id.to_owned() }
  }
}

pub enum EmoteResponse {
  GlobalEmoteImageLoaded { name : String, data: Option<Vec<(ColorImage, u16)>> },
  GlobalBadgeImageLoaded { name : String, data: Option<Vec<(ColorImage, u16)>> },
  ChannelEmoteImageLoaded { name : String, channel_name: String, data: Option<Vec<(ColorImage, u16)>> },
  ChannelBadgeImageLoaded { name : String, channel_name: String, data: Option<Vec<(ColorImage, u16)>> },
  TwitchMsgEmoteLoaded { name: String, id: String, data: Option<Vec<(ColorImage, u16)>> }
}

#[derive(Default)]
pub enum EmoteStatus {
  #[default]
  NotLoaded,
  Loading,
  Loaded,
}

#[derive(Clone, Debug)]
pub struct CssAnimationData {
  pub width: u32,
  pub cycle_time_msec: isize,
  pub steps: isize
}

#[derive(Default)]
pub struct Emote {
  pub name: String,
  pub id: String,
  pub display_name: Option<String>,
  pub color: Option<(u8,u8,u8)>,
  pub data: Option<Vec<(TextureHandle, u16)>>,
  pub loaded: EmoteStatus,
  pub duration_msec: u16,
  pub url: String,
  pub path: String,
  pub extension: Option<String>,
  pub zero_width: bool,
  pub css_anim: Option<CssAnimationData>,
  pub priority: isize
}

pub struct EmoteLoader {
  pub tx: async_channel::Sender<EmoteRequest>,
  pub rx: Receiver<EmoteResponse>,
  handle: Vec<JoinHandle<()>>,
  pub transparent_img: Option<TextureHandle>,
  pub base_path: PathBuf
}

impl EmoteLoader {
  pub fn new(app_name: &str, runtime: &Runtime) -> Self {
    let (in_tx, in_rx) = async_channel::unbounded::<EmoteRequest>();
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<EmoteResponse>(256);

    let mut tasks : Vec<JoinHandle<()>> = Vec::new();
    for n in 1..5 {
      let base_path = cache_path_from_app_name(app_name).expect("Failed to locate an appropiate location to store cache files");
      let in_rx = in_rx.clone();
      let out_tx = out_tx.clone();
      let n = n;
      let task : JoinHandle<()> = runtime.spawn(async move { 
        println!("emote thread {n}");
        let mut easy = Easy::new();
        loop {
          let recv_msg = in_rx.recv().await;
          if let Ok(msg) = recv_msg {
            let sent_msg = match msg {
              EmoteRequest::ChannelEmoteImage { name, id, url, path, extension, channel_name, css_anim } => {
                //println!("{n} loading channel emote {} '{}' for {}", name, url, channel_name);
                let data = imaging::get_image_data(&url, base_path.join(path), &id, &extension, &mut easy, css_anim);
                out_tx.try_send(EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data })
              },
              EmoteRequest::ChannelBadgeImage { name, id, url, path, extension, channel_name } => {
                //println!("{n} loading channel badge {} '{}' for {}", name, url, channel_name);
                let data = imaging::get_image_data(&url, base_path.join(path), &id, &extension, &mut easy, None);
                out_tx.try_send(EmoteResponse::ChannelBadgeImageLoaded { name, channel_name, data })
              },
              EmoteRequest::GlobalEmoteImage { name, id, url, path, extension } => {
                //println!("{n} loading global emote {} '{}'", name, url);
                let data = imaging::get_image_data(&url, base_path.join(path), &id, &extension, &mut easy, None);
                out_tx.try_send(EmoteResponse::GlobalEmoteImageLoaded { name, data })
              },
              EmoteRequest::GlobalBadgeImage { name, id, url, path, extension } => {
                //println!("{n} loading global badge {}", name);
                let data = imaging::get_image_data(&url, base_path.join(path), &id, &extension, &mut easy, None);
                out_tx.try_send(EmoteResponse::GlobalBadgeImageLoaded { name, data })
              },
              EmoteRequest::TwitchMsgEmoteImage { name, id } => {
                //println!("{n} loading twitch emote {} '{}'", name, id);
                let mut data = imaging::get_image_data(&format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/animated/light/3.0", id), base_path.join("cache/twitch/"), &id, &None, &mut easy, None);
                if data.is_none() {
                  data = imaging::get_image_data(&format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/static/light/3.0", id), base_path.join("cache/twitch/"), &id, &None, &mut easy, None)
                }
                out_tx.try_send(EmoteResponse::TwitchMsgEmoteLoaded { name, id, data })
              }
            };
            match sent_msg {
              Ok(()) => (),
              Err(e) => println!("Error sending loaded image event: {}", e)
            };
          }
          // everything ends up handled by one thread without this delay
          tokio::time::sleep(Duration::from_millis(10)).await;
        }
      });
      tasks.insert(tasks.len(), task);
    }

    //println!("counted {} receivers", in_rx.receiver_count());
    //in_rx.close();

    Self { 
      tx: in_tx,
      rx: out_rx,
      handle: tasks,
      transparent_img: None,
      base_path: cache_path_from_app_name(app_name).expect("Failed to locate an appropiate location to store cache files")
     }
  }

  pub fn close(&self) {
    self.handle.iter().for_each(|x| x.abort());
  }

  pub fn load_channel_emotes(
    &mut self,
    channel_id: &String,
    token: &String
  ) -> std::result::Result<HashMap<String, Emote>, anyhow::Error> {
    let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
    let ffz_emotes = self.process_emote_json(
      &ffz_url,
      &format!("cache/ffz-channel-json-{}", channel_id),
      None
    )?;
    let bttv_url = format!(
      "https://api.betterttv.net/3/cached/users/twitch/{}",
      channel_id
    );
    let bttv_emotes = self.process_emote_json(
      &bttv_url,
      &format!("cache/bttv-channel-json-{}", channel_id),
      None
    )?;
    let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
    let seventv_emotes = self.process_emote_json(
      &seventv_url,
      &format!("cache/7tv-channel-json-{}", channel_id),
      None
    )?;
    let twitch_url = format!("https://api.twitch.tv/helix/chat/emotes?broadcaster_id={}", channel_id);
    let twitch_follower_emotes = self.process_twitch_follower_emote_json(
      &twitch_url,
      &format!("cache/twitch-{}", channel_id),
      Some([
        ("Authorization", &format!("Bearer {}", token)),
        ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())].to_vec())
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
    for emote in twitch_follower_emotes {
      result.insert(emote.name.to_owned(), emote);
    }
    Ok(result)
  }

  pub fn load_global_emotes(
    &self,
  ) -> std::result::Result<HashMap<String, Emote>, anyhow::Error> {
    let bttv_emotes = self.process_emote_json(
      "https://api.betterttv.net/3/cached/emotes/global",
      "cache/bttv-global-json",
      None
    )?;
    let seventv_emotes = self.process_emote_json(
      "https://api.7tv.app/v2/emotes/global",
      "cache/7tv-global-json",
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

  fn process_emote_json(&self, url: &str, path: &str, headers: Option<Vec<(&str, &String)>>) -> std::result::Result<Vec<Emote>, anyhow::Error> {
    fetch::process_emote_json(url, self.base_path.join(path).to_str().unwrap(), headers)
  }

  fn process_twitch_follower_emote_json(&self, twitch_url: &str, path: &str, headers: Option<Vec<(&str, &String)>>) -> std::result::Result<Vec<Emote>, anyhow::Error> {
    fetch::process_twitch_follower_emote_json(twitch_url, self.base_path.join(path).to_str().unwrap(), headers)
  }

  fn process_badge_json(&self, room_id: &str, url: &str, filename: &str, headers: Option<Vec<(&str, &String)>>) -> std::result::Result<Vec<Emote>, anyhow::Error> {
    fetch::process_badge_json(room_id, url, self.base_path.join(filename).to_str().unwrap(), headers)
  }

  pub fn twitch_get_emote_set(&mut self, token : &String, emote_set_id : &String) -> Option<HashMap<String, Emote>> { 
    if emote_set_id.contains(':') || emote_set_id.contains('-') || emote_set_id.contains("emotesv2") {
      return None;
    }

    let emotes = self.process_emote_json(
      &format!("https://api.twitch.tv/helix/chat/emotes/set?emote_set_id={}", emote_set_id),
      &format!("cache/twitch-emote-set-{}", emote_set_id),
      Some([
        ("Authorization", &format!("Bearer {}", token)),
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

  pub fn twitch_get_global_badges(&self, token : &String) -> Option<HashMap<String, Emote>> { 
    let emotes = self.process_badge_json(
      "global",
      "https://api.twitch.tv/helix/chat/badges/global",
      "cache/twitch-badges-global",
      Some([
        ("Authorization", &format!("Bearer {}", token)),
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

  pub fn twitch_get_channel_badges(&self, token : &String, room_id : &String) -> Option<HashMap<String, Emote>> { 
    let emotes = self.process_badge_json(
      room_id,
      &format!("https://api.twitch.tv/helix/chat/badges?broadcaster_id={}", room_id),
      &format!("cache/twitch-badges-channel-{}", room_id),
      Some([
        ("Authorization", &format!("Bearer {}", token)),
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
        println!("Error loading channel badge json: {}", e);
        Some(HashMap::new())
      }
    }
  }
}

pub fn cache_path_from_app_name(app_name: &str) -> Option<PathBuf> {
  // Lifted from egui
  if let Some(proj_dirs) = directories_next::ProjectDirs::from("", "", app_name) {
      let data_dir = proj_dirs.data_dir().to_path_buf();
      if let Err(err) = std::fs::create_dir_all(&data_dir) {
          println!(
              "Saving disabled: Failed to create app path at {:?}: {}",
              data_dir,
              err
          );
          None
      } else {
          Some(data_dir)
      }
  } else {
      println!("Saving disabled: Failed to find path to data_dir.");
      None
  }
}