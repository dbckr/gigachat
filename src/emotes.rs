/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use curl::easy::Easy;
use eframe::{
  epaint::{TextureHandle},
};
use failure;


use ::image::DynamicImage;
use tokio::{runtime::Runtime, sync::mpsc::{Receiver}, task::JoinHandle};
use std::{collections::HashMap, time::Duration};
use std::str;

use crate::provider::ProviderName;

pub mod fetch;
pub mod imaging;

pub enum EmoteRequest {
  GlobalEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  GlobalBadgeImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  ChannelEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String },
  ChannelBadgeImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String },
  EmoteSetImage { name: String, id : String, url: String, path: String, extension: Option<String>, set_id: String, provider_name: ProviderName },
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
      channel_name: channel_name.to_owned()
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
  GlobalBadgeImageLoaded { name : String, data: Option<Vec<(DynamicImage, u16)>> },
  ChannelEmoteImageLoaded { name : String, channel_name: String, data: Option<Vec<(DynamicImage, u16)>> },
  ChannelBadgeImageLoaded { name : String, channel_name: String, data: Option<Vec<(DynamicImage, u16)>> },
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
  pub tx: async_channel::Sender<EmoteRequest>,
  pub rx: Receiver<EmoteResponse>,
  handle: Vec<JoinHandle<()>>,
  pub transparent_img: Option<TextureHandle>
}

impl EmoteLoader {
  pub fn new(runtime: &Runtime) -> Self {
    let (in_tx, in_rx) = async_channel::unbounded(); //tokio::sync::mpsc::channel::<EmoteRequest>(128);
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<EmoteResponse>(256);

    let mut tasks : Vec<JoinHandle<()>> = Vec::new();
    for n in 1..5 {
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
              EmoteRequest::ChannelEmoteImage { name, id, url, path, extension, channel_name } => {
                println!("{n} loading channel emote {} for {}", name, channel_name);
                let data = imaging::get_image_data(&url, &path, &id, &extension, &mut easy);
                out_tx.try_send(EmoteResponse::ChannelEmoteImageLoaded { name: name, channel_name: channel_name, data: data })
              },
              EmoteRequest::ChannelBadgeImage { name, id, url, path, extension, channel_name } => {
                println!("{n} loading channel badge {} for {}", name, channel_name);
                let data = imaging::get_image_data(&url, &path, &id, &extension, &mut easy);
                out_tx.try_send(EmoteResponse::ChannelBadgeImageLoaded { name: name, channel_name: channel_name, data: data })
              },
              EmoteRequest::GlobalEmoteImage { name, id, url, path, extension } => {
                println!("{n} loading global emote {}", name);
                let data = imaging::get_image_data(&url, &path, &id, &extension, &mut easy);
                out_tx.try_send(EmoteResponse::GlobalEmoteImageLoaded { name: name, data: data })
              },
              EmoteRequest::GlobalBadgeImage { name, id, url, path, extension } => {
                println!("{n} loading global badge {}", name);
                let data = imaging::get_image_data(&url, &path, &id, &extension, &mut easy);
                out_tx.try_send(EmoteResponse::GlobalBadgeImageLoaded { name: name, data: data })
              },
              EmoteRequest::EmoteSetImage { name, id, url, path, extension, set_id, provider_name } => {
                println!("{n} loading set emote {} for set {}", name, set_id);
                let data = imaging::get_image_data(&url, &path, &id, &extension, &mut easy);
                let data_copy = data.clone();
                out_tx.try_send(EmoteResponse::TwitchMsgEmoteLoaded { name: name.to_owned(), id: id, data: data })
                  .or(out_tx.try_send(EmoteResponse::EmoteSetImageLoaded { name: name, provider_name: provider_name, set_id: set_id, data: data_copy }))
              },
              EmoteRequest::TwitchMsgEmoteImage { name, id } => {
                println!("{n} loading twitch emote {} '{}'", name, id);
                let data = if let Some(x) = imaging::get_image_data(&format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/animated/light/3.0", id), "generated/twitch/", &id, &None, &mut easy) {
                  Some(x)
                } else {
                  imaging::get_image_data(&format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/static/light/3.0", id), "generated/twitch/", &id, &None, &mut easy)
                };
                out_tx.try_send(EmoteResponse::TwitchMsgEmoteLoaded { name: name, id: id, data: data })
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
      transparent_img: None
     }
  }

  pub fn close(&self) {
    self.handle.iter().for_each(|x| x.abort());
  }

  pub fn load_channel_emotes(
    &mut self,
    channel_id: &String,
  ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
    let ffz_emotes = fetch::process_emote_json(
      &ffz_url,
      &format!("generated/ffz-channel-json-{}", channel_id),
      None
    )?;
    let bttv_url = format!(
      "https://api.betterttv.net/3/cached/users/twitch/{}",
      channel_id
    );
    let bttv_emotes = fetch::process_emote_json(
      &bttv_url,
      &format!("generated/bttv-channel-json-{}", channel_id),
      None
    )?;
    let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
    let seventv_emotes = fetch::process_emote_json(
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
    &self,
  ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let bttv_emotes = fetch::process_emote_json(
      "https://api.betterttv.net/3/cached/emotes/global",
      "generated/bttv-global-json",
      None
    )?;
    let seventv_emotes = fetch::process_emote_json(
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

  pub fn twitch_get_emote_set(&mut self, token : &String, emote_set_id : &String) -> Option<HashMap<String, Emote>> { 
    if emote_set_id.contains(":") || emote_set_id.contains("-") || emote_set_id.contains("emotesv2") {
      return None;
    }

    let emotes = fetch::process_emote_json(
      &format!("https://api.twitch.tv/helix/chat/emotes/set?emote_set_id={}", emote_set_id),
      &format!("generated/twitch-emote-set-{}", emote_set_id),
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
    let emotes = fetch::process_badge_json(
      "global",
      &format!("https://api.twitch.tv/helix/chat/badges/global"),
      &format!("generated/twitch-badges-global"),
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
    let emotes = fetch::process_badge_json(
      room_id,
      &format!("https://api.twitch.tv/helix/chat/badges?broadcaster_id={}", room_id),
      &format!("generated/twitch-badges-channel-{}", room_id),
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
}