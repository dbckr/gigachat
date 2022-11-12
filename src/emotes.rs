/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use async_channel::Receiver;
use chrono::{DateTime, Utc};
use tracing::{info, debug, warn, error};
use curl::easy::Easy;
use egui::{epaint::{TextureHandle}};
use egui::ColorImage;

use tokio::{runtime::Runtime, task::JoinHandle};
use std::{collections::{HashMap}, time::Duration, path::{PathBuf, Path}};
use std::str;
use tracing_unwrap::{OptionExt};

use crate::{provider::{dgg, channel::{ChannelShared, Channel}, Provider}, TemplateApp};

pub mod fetch;
pub mod imaging;

pub enum EmoteRequest {
  GlobalEmoteListRequest { force_redownload: bool },
  GlobalEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  TwitchGlobalBadgeListRequest { token: String, force_redownload: bool },
  GlobalBadgeImage { name: String, id : String, url: String, path: String, extension: Option<String> },
  ChannelEmoteImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String, css_anim: Option<CssAnimationData> },
  ChannelBadgeImage { name: String, id : String, url: String, path: String, extension: Option<String>, channel_name: String },
  TwitchMsgEmoteImage { name: String, id: String },
  TwitchBadgeEmoteListRequest { channel_id: String, channel_name: String, token: String, force_redownload: bool },
  TwitchEmoteSetRequest { token: String, emote_set_id: String, force_redownload: bool },
  DggFlairEmotesRequest { channel_name: String, cdn_base_url: String, force_redownload: bool },
  YouTubeMsgEmoteImage { name: String, url: String, path: String },
  //JsonDownloadRequest { url: String, filename: String, headers: Option<Vec<(String, String)>> }
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
  pub fn new_youtube_emote_request(emote: &Emote) -> Self {
    EmoteRequest::YouTubeMsgEmoteImage { name: emote.name.to_owned(), url: emote.url.to_owned(), path: emote.path.to_owned() }
  }
}

pub enum EmoteResponse {
  GlobalEmoteImageLoaded { name : String, data: Option<Vec<(ColorImage, u16)>> },
  GlobalBadgeImageLoaded { name : String, data: Option<Vec<(ColorImage, u16)>> },
  ChannelEmoteImageLoaded { name : String, channel_name: String, data: Option<Vec<(ColorImage, u16)>> },
  ChannelBadgeImageLoaded { name : String, channel_name: String, data: Option<Vec<(ColorImage, u16)>> },
  TwitchMsgEmoteLoaded { name: String, id: String, data: Option<Vec<(ColorImage, u16)>> },
  ChannelEmoteListResponse { channel_name: String, response: Result<HashMap<String, Emote>, anyhow::Error> },
  ChannelBadgeListResponse { channel_name: String, response: Result<HashMap<String, Emote>, anyhow::Error> },
  TwitchEmoteSetResponse { emote_set_id: String, response: Result<HashMap<String, Emote>, anyhow::Error> },
  //JsonDownloadResponse { url: String, filename: String, content: String }
  GlobalEmoteListResponse { response: Result<HashMap<String, Emote>, anyhow::Error> },
  TwitchGlobalBadgeListResponse { response: Result<HashMap<String, Emote>, anyhow::Error> },
  YouTubeMsgEmoteLoaded { name: String, data: Option<Vec<(ColorImage, u16)>> },
}

#[derive(Default)]
pub enum EmoteStatus {
  #[default]
  NotLoaded,
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
  pub priority: isize,
  pub hidden: bool,
  pub texture_expiration: Option<DateTime<Utc>>
}

pub struct EmoteLoader {
  pub tx: async_channel::Sender<EmoteRequest>,
  pub rx: Receiver<EmoteResponse>,
  handle: Vec<JoinHandle<()>>,
  pub transparent_img: Option<TextureHandle>,
  pub base_path: PathBuf,
  pub loading_emotes: HashMap<String, DateTime<Utc>>
}

impl Default for EmoteLoader {
  fn default() -> Self {
    Self { 
      tx: async_channel::unbounded::<EmoteRequest>().0,
      rx: async_channel::unbounded::<EmoteResponse>().1, 
      handle: Default::default(), 
      transparent_img: None,
      base_path: Default::default(), 
      loading_emotes: Default::default() 
    }
  }
}

impl EmoteLoader {
  pub fn new(app_name: &str, runtime: &Runtime) -> Self {
    let (in_tx, in_rx) = async_channel::unbounded::<EmoteRequest>();
    let (out_tx, out_rx) = async_channel::unbounded::<EmoteResponse>();

    let mut tasks : Vec<JoinHandle<()>> = Vec::new();
    for n in 1..9 {
      let cache_path = cache_path_from_app_name(app_name).expect_or_log("Failed to locate an appropiate location to store cache files");
      let in_rx = in_rx.clone();
      let out_tx = out_tx.clone();
      let n = n;
      let task : JoinHandle<()> = runtime.spawn(async move { 
        debug!("starting emote thread {n}");
        let mut easy = Easy::new();
        loop {
          let recv_msg = in_rx.recv().await;
          if let Ok(msg) = recv_msg {
            let sent_msg = match msg {
              EmoteRequest::ChannelEmoteImage { name, id, url, path, extension, channel_name, css_anim } => {
                //info!("{n} loading channel emote {} '{}' for {}", name, url, channel_name);
                let data = imaging::get_image_data(&name, &url, cache_path.join(path), &id, &extension, &mut easy, css_anim);
                out_tx.try_send(EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data })
              },
              EmoteRequest::ChannelBadgeImage { name, id, url, path, extension, channel_name } => {
                //info!("{n} loading channel badge {} '{}' for {}", name, url, channel_name);
                let data = imaging::get_image_data(&name, &url, cache_path.join(path), &id, &extension, &mut easy, None);
                out_tx.try_send(EmoteResponse::ChannelBadgeImageLoaded { name, channel_name, data })
              },
              EmoteRequest::GlobalEmoteImage { name, id, url, path, extension } => {
                //info!("{n} loading global emote {} '{}'", name, url);
                let data = imaging::get_image_data(&name, &url, cache_path.join(path), &id, &extension, &mut easy, None);
                out_tx.try_send(EmoteResponse::GlobalEmoteImageLoaded { name, data })
              },
              EmoteRequest::GlobalBadgeImage { name, id, url, path, extension } => {
                //info!("{n} loading global badge {}", name);
                let data = imaging::get_image_data(&name, &url, cache_path.join(path), &id, &extension, &mut easy, None);
                out_tx.try_send(EmoteResponse::GlobalBadgeImageLoaded { name, data })
              },
              EmoteRequest::TwitchMsgEmoteImage { name, id } => {
                //info!("{n} loading twitch emote {} '{}'", name, id);
                let mut data = imaging::get_image_data(&name, &format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/animated/light/3.0", id), cache_path.join("twitch/"), &id, &None, &mut easy, None);
                if data.is_none() {
                  data = imaging::get_image_data(&name, &format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/static/light/3.0", id), cache_path.join("twitch/"), &id, &None, &mut easy, None)
                }
                out_tx.try_send(EmoteResponse::TwitchMsgEmoteLoaded { name, id, data })
              },
              EmoteRequest::YouTubeMsgEmoteImage { name, url, path } => {
                //info!("{n} loading youtube emote '{}'", name);
                let data = imaging::get_image_data(&name, &url, cache_path.join(path), &name, &None, &mut easy, None);
                out_tx.try_send(EmoteResponse::YouTubeMsgEmoteLoaded { name, data })
              },
              EmoteRequest::TwitchEmoteSetRequest { token, emote_set_id, force_redownload } => {
                let data = twitch_get_emote_set(&token, &emote_set_id, &cache_path, force_redownload);
                out_tx.try_send(EmoteResponse::TwitchEmoteSetResponse { emote_set_id, response: data })
              },
              EmoteRequest::TwitchBadgeEmoteListRequest { channel_id, channel_name, token, force_redownload } => {
                let data = load_channel_emotes(&channel_id, &token, &cache_path, force_redownload);
                match out_tx.try_send(EmoteResponse::ChannelEmoteListResponse { channel_name: channel_name.to_owned(), response: data }) {
                  Ok(()) => (),
                  Err(e) => warn!("Error sending event: {}", e)
                };
                let badge_list = twitch_get_channel_badges(&token, &channel_id, &cache_path, force_redownload);
                out_tx.try_send(EmoteResponse::ChannelBadgeListResponse { channel_name, response: badge_list })
              },
              EmoteRequest::DggFlairEmotesRequest { channel_name, cdn_base_url, force_redownload } => {
                let emote_list = dgg::load_dgg_emotes(&cdn_base_url, &cache_path, force_redownload);
                match out_tx.try_send(EmoteResponse::ChannelEmoteListResponse { channel_name: channel_name.to_owned(), response: emote_list }) {
                  Ok(()) => (),
                  Err(e) => warn!("Error sending event: {}", e)
                };
                let badge_list = dgg::load_dgg_flairs(&cdn_base_url, &cache_path, force_redownload);
                out_tx.try_send(EmoteResponse::ChannelBadgeListResponse { channel_name, response: badge_list })
              },
              EmoteRequest::GlobalEmoteListRequest { force_redownload } => {
                let data = load_global_emotes(&cache_path, force_redownload);
                out_tx.try_send(EmoteResponse::GlobalEmoteListResponse { response: data })
              },
              EmoteRequest::TwitchGlobalBadgeListRequest { token, force_redownload } => {
                let data = twitch_get_global_badges(&token, &cache_path, force_redownload);
                out_tx.try_send(EmoteResponse::TwitchGlobalBadgeListResponse { response: data })
              }
            };
            match sent_msg {
              Ok(()) => (),
              Err(e) => warn!("Error sending event: {}", e)
            };
          }
          // everything ends up handled by one thread without this delay
          tokio::time::sleep(Duration::from_millis(10)).await;
        }
      });
      tasks.insert(tasks.len(), task);
    }

    //info!("counted {} receivers", in_rx.receiver_count());
    //in_rx.close();

    Self { 
      tx: in_tx,
      rx: out_rx,
      handle: tasks,
      transparent_img: None,
      base_path: cache_path_from_app_name(app_name).expect_or_log("Failed to locate an appropiate location to store cache files"),
      loading_emotes: Default::default()
     }
  }

  pub fn close(&self) {
    self.handle.iter().for_each(|x| x.abort());
  }  
}

pub fn load_channel_emotes(
  channel_id: &String,
  token: &String,
  cache_path: &Path,
  force_redownload: bool
) -> std::result::Result<HashMap<String, Emote>, anyhow::Error> {
  let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
  let ffz_emotes = process_emote_json(
    &ffz_url,
    cache_path,
    &format!("ffz-channel-json-{}", channel_id),
    None,
    force_redownload
  )?;
  let bttv_url = format!(
    "https://api.betterttv.net/3/cached/users/twitch/{}",
    channel_id
  );
  let bttv_emotes = process_emote_json(
    &bttv_url,
    cache_path,
    &format!("bttv-channel-json-{}", channel_id),
    None,
    force_redownload
  )?;
  let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
  let seventv_emotes = process_emote_json(
    &seventv_url,
    cache_path,
    &format!("7tv-channel-json-{}", channel_id),
    None,
    force_redownload
  )?;
  let twitch_url = format!("https://api.twitch.tv/helix/chat/emotes?broadcaster_id={}", channel_id);
  let twitch_follower_emotes = process_twitch_follower_emote_json(
    &twitch_url,
    cache_path,
    &format!("twitch-{}", channel_id),
    Some([
      ("Authorization", &format!("Bearer {}", token)),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())].to_vec()
    ),
    force_redownload
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
  cache_path: &Path,
  force_redownload: bool
) -> std::result::Result<HashMap<String, Emote>, anyhow::Error> {
  let bttv_emotes = process_emote_json(
    "https://api.betterttv.net/3/cached/emotes/global",
    cache_path,
    "bttv-global-json",
    None,
    force_redownload
  )?;
  let seventv_emotes = process_emote_json(
    "https://api.7tv.app/v2/emotes/global",
    cache_path,
    "7tv-global-json",
    None,
    force_redownload
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

//self.base_path.join(path).to_str().unwrap_or_log()
//self.base_path.join(filename).to_str().unwrap_or_log()
fn process_emote_json(url: &str, cache_path: &Path, path: &str, headers: Option<Vec<(&str, &String)>>, force_redownload: bool) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  fetch::process_emote_json(url, cache_path.join(path).to_str().unwrap_or_log(), headers, force_redownload)
}

fn process_twitch_follower_emote_json(twitch_url: &str, cache_path: &Path, path: &str, headers: Option<Vec<(&str, &String)>>, force_redownload: bool) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  fetch::process_twitch_follower_emote_json(twitch_url, cache_path.join(path).to_str().unwrap_or_log(), headers, force_redownload)
}

fn process_badge_json(room_id: &str, url: &str, cache_path: &Path, path: &str, headers: Option<Vec<(&str, &String)>>, force_redownload: bool) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  fetch::process_badge_json(room_id, url, cache_path.join(path).to_str().unwrap_or_log(), headers, force_redownload)
}

pub fn twitch_get_emote_set(token : &String, emote_set_id : &String, cache_path: &Path, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> { 
  if emote_set_id.contains(':') || emote_set_id.contains('-') || emote_set_id.contains("emotesv2") {
    return Ok(Default::default());
  }

  let emotes = process_emote_json(
    &format!("https://api.twitch.tv/helix/chat/emotes/set?emote_set_id={}", emote_set_id),
    cache_path,
    &format!("twitch-emote-set-{}", emote_set_id),
    Some([
      ("Authorization", &format!("Bearer {}", token)),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
    ].to_vec()),
    force_redownload
  );

  match emotes {
    Ok(emotes) => {
      let mut map = HashMap::new();
      for emote in emotes {
        map.insert(emote.name.to_owned(), emote);
      }
      Ok(map)
    },
    Err(e) => {
      info!("Error loading emote set: {}", e);
      Err(e)
    }
  }
}

pub fn twitch_get_global_badges(token : &String, cache_path: &Path, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> { 
  let emotes = process_badge_json(
    "global",
    "https://api.twitch.tv/helix/chat/badges/global",
    cache_path,
    "twitch-badges-global",
    Some([
      ("Authorization", &format!("Bearer {}", token)),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
    ].to_vec()),
    force_redownload
  );

  match emotes {
    Ok(emotes) => {
      let mut map = HashMap::new();
      for emote in emotes {
        map.insert(emote.name.to_owned(), emote);
      }
      Ok(map)
    },
    Err(e) => Err(e)
  }
}

pub fn twitch_get_channel_badges(token : &String, room_id : &String, cache_path: &Path, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> { 
  let emotes = process_badge_json(
    room_id,
    &format!("https://api.twitch.tv/helix/chat/badges?broadcaster_id={}", room_id),
    cache_path,
    &format!("twitch-badges-channel-{}", room_id),
    Some([
      ("Authorization", &format!("Bearer {}", token)),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
    ].to_vec()),
    force_redownload
  );

  match emotes {
    Ok(emotes) => {
      let mut map = HashMap::new();
      for emote in emotes {
        map.insert(emote.name.to_owned(), emote);
      }
      Ok(map)
    },
    Err(e) => Err(e)
  }
}

pub fn cache_path_from_app_name(app_name: &str) -> Option<PathBuf> {
  // Lifted from egui
  if let Some(proj_dirs) = directories_next::ProjectDirs::from("", "", app_name) {
      let data_dir = proj_dirs.cache_dir().to_path_buf();
      if let Err(err) = std::fs::create_dir_all(&data_dir) {
          info!(
              "Saving disabled: Failed to create app path at {:?}: {}",
              data_dir,
              err
          );
          None
      } else {
          Some(data_dir)
      }
  } else {
      info!("Saving disabled: Failed to find path to data_dir.");
      None
  }
}

fn load_emote_data(emote: &mut Emote, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>) {
  emote.data = imaging::load_to_texture_handles(ctx, data);
  emote.duration_msec = match emote.data.as_ref() {
    Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
    _ => 0,
  };
  emote.loaded = EmoteStatus::Loaded;
  emote.texture_expiration = None;//Some(chrono::Utc::now().add(chrono::Duration::hours(12)));
  loading_emotes.remove(&emote.name);
}

pub trait LoadEmote {
  fn update_emote(&mut self, emote_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>);
  fn update_badge(&mut self, badge_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>);
}

impl LoadEmote for ChannelShared {
  fn update_emote(&mut self, emote_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>) {
    if let Some(transient) = self.transient.as_mut() && let Some(emotes) = transient.channel_emotes.as_mut() && let Some(emote) = emotes.get_mut(emote_name) {
      load_emote_data(emote, ctx, data, loading_emotes);
    }
  }
  fn update_badge(&mut self, badge_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>) {
    if let Some(transient) = self.transient.as_mut() && let Some(badges) = transient.badge_emotes.as_mut() && let Some(emote) = badges.get_mut(badge_name) {
      load_emote_data(emote, ctx, data, loading_emotes);
    }
  }
}

impl TemplateApp {
  pub fn update_emote(&mut self, emote_name: &String, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>) {
    if let Some(emote) = self.global_emotes.get_mut(emote_name) {
      emote.data = imaging::load_to_texture_handles(ctx, data);
      emote.duration_msec = match emote.data.as_ref() {
        Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
        _ => 0,
      };
      emote.loaded = EmoteStatus::Loaded;
      emote.texture_expiration = None;//Some(chrono::Utc::now().add(chrono::Duration::hours(12)));
      self.emote_loader.loading_emotes.remove(&emote.name);
    }
  }
}

impl LoadEmote for Provider {
  fn update_emote(&mut self, emote_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>) {
    if let Some(emote) = self.emotes.get_mut(emote_name) {
      emote.data = imaging::load_to_texture_handles(ctx, data);
      emote.duration_msec = match emote.data.as_ref() {
        Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
        _ => 0,
      };
      emote.loaded = EmoteStatus::Loaded;
      emote.texture_expiration = None;//Some(chrono::Utc::now().add(chrono::Duration::hours(12)));
      loading_emotes.remove(&emote.name);
    }
  }
  fn update_badge(&mut self, badge_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>) {
    if let Some(global_badges) = &mut self.global_badges && let Some(emote) = global_badges.get_mut(badge_name) {
      emote.data = imaging::load_to_texture_handles(ctx, data);
      emote.duration_msec = match emote.data.as_ref() {
        Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
        _ => 0,
      };
      emote.loaded = EmoteStatus::Loaded;
      emote.texture_expiration = None;//Some(chrono::Utc::now().add(chrono::Duration::hours(12)));
      loading_emotes.remove(&emote.name);
    }
  }
}

pub trait AddEmote {
  fn set_emotes(&mut self, emotes : Result<HashMap<String, Emote>, anyhow::Error>);
  fn set_badges(&mut self, emotes : Result<HashMap<String, Emote>, anyhow::Error>);
}

impl AddEmote for Channel {
  fn set_emotes(&mut self, emotes : Result<HashMap<String, Emote>, anyhow::Error>) {
    match emotes {
      Ok(emotes) => {
        let shared = match self {
          Channel::DGG { dgg: _, ref mut shared } => shared,
          Channel::Twitch { twitch: _, ref mut shared } => shared,
          Channel::Youtube { youtube: _, ref mut shared } => shared
        };
        if let Some(t) = shared.transient.as_mut() {
          t.channel_emotes = Some(emotes)
        }
      },
      Err(e) => { error!("Failed to load emote json for channel {} due to error {:?}", self.channel_name(), e); }
    }
  }
  fn set_badges(&mut self, badges : Result<HashMap<String, Emote>, anyhow::Error>) {
    match badges {
      Ok(badges) => {
        let shared = match self {
          Channel::DGG { dgg: _, ref mut shared } => shared,
          Channel::Twitch { twitch: _, ref mut shared } => shared,
          Channel::Youtube { youtube: _, ref mut shared } => shared
        };
        if let Some(t) = shared.transient.as_mut() {
          t.badge_emotes = Some(badges)
        }
      },
      Err(e) => { error!("Failed to load badge json for channel {} due to error {:?}", self.channel_name(), e); }
    }
  }
}