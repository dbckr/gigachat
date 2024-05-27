/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use ahash::HashSet;
use async_channel::Receiver;
use chrono::Timelike;
use tracing::{debug, info, warn, error};
use egui::{ColorImage, Context, TextureHandle};

use tokio::{runtime::Runtime, task::JoinHandle};
use std::{collections::HashMap, path::{Path, PathBuf}};
use std::str;
use tracing_unwrap::OptionExt;

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

pub struct OverlayItem<'a> {
  pub name: &'a String,
  pub texture: Option<&'a TextureHandle>
}

pub struct EmoteFrame<'a> {
  pub id: &'a String,
  pub name: &'a String,
  pub path: &'a String,
  pub label: &'a Option<String>,
  pub texture: Option<&'a TextureHandle>,
  pub zero_width: bool
}

#[derive(Default)]
pub enum EmoteSource
{
  #[default]
  Channel,
  Global,
  ChannelBadge,
  GlobalBadge,
  Twitch,
  Youtube
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
  pub source: EmoteSource,
  pub channel_name: String
}

impl Emote {
  pub fn get_overlay_item(&self, emote_loader: &mut EmoteLoader, ctx: &Context) -> OverlayItem {
    OverlayItem { name: &self.name, texture: self.get_texture3(emote_loader, ctx) }
  }

  fn get_emote_request(&self) -> EmoteRequest {
    let emote = self;
    match self.source {
      EmoteSource::Channel => EmoteRequest::ChannelEmoteImage { 
        name: emote.name.to_owned(),
        id: emote.id.to_owned(), 
        url: emote.url.to_owned(), 
        path: emote.path.to_owned(), 
        extension: emote.extension.to_owned(), 
        channel_name: emote.channel_name.to_owned(),
        css_anim: emote.css_anim.clone()
      },
      EmoteSource::Global => EmoteRequest::GlobalEmoteImage {
        name: emote.name.to_owned(),
        id: emote.id.to_owned(), 
        url: emote.url.to_owned(), 
        path: emote.path.to_owned(), 
        extension: emote.extension.to_owned()
      },
      EmoteSource::ChannelBadge => EmoteRequest::ChannelBadgeImage { 
        name: emote.name.to_owned(),
        id: emote.id.to_owned(), 
        url: emote.url.to_owned(), 
        path: emote.path.to_owned(), 
        extension: emote.extension.to_owned(), 
        channel_name: emote.channel_name.to_owned()
      },
      EmoteSource::GlobalBadge => EmoteRequest::GlobalBadgeImage {
        name: emote.name.to_owned(),
        id: emote.id.to_owned(), 
        url: emote.url.to_owned(), 
        path: emote.path.to_owned(), 
        extension: emote.extension.to_owned()
      },
      EmoteSource::Twitch => EmoteRequest::TwitchMsgEmoteImage { 
        name: emote.name.to_owned(), 
        id: emote.id.to_owned() 
      },
      EmoteSource::Youtube => EmoteRequest::YouTubeMsgEmoteImage { 
        name: emote.name.to_owned(), 
        url: emote.url.to_owned(), 
        path: emote.path.to_owned() 
      }
    }
  }

  pub fn get_texture3<'a>(&'a self, emote_loader: &mut EmoteLoader, ctx: &Context) -> Option<&'a TextureHandle> {
    let emote = self;
    match emote.loaded {
      EmoteStatus::NotLoaded => {
        emote_loader.request_emote(&emote.name, emote.get_emote_request());
        None
      },
      EmoteStatus::Loaded => {
        get_texture(emote, ctx)
      }
    }
  }

  pub fn get_texture2(&self, ctx: &Context) -> Option<&TextureHandle> {
    let emote = self;
    match emote.loaded {
      EmoteStatus::NotLoaded => {
        None
      },
      EmoteStatus::Loaded => {
        get_texture(emote, ctx)
      }
    }
  }

  pub fn get_texture<'a>(&'a self, emote_loader: &'a mut EmoteLoader, ctx: &Context) -> Option<&'a TextureHandle> {
    let emote = self;
    match emote.loaded {
      EmoteStatus::NotLoaded => {
        emote_loader.request_emote(&emote.name, emote.get_emote_request());
        emote_loader.transparent_img.as_ref()
      },
      EmoteStatus::Loaded => {
        get_texture(emote, ctx)
      }
    }
  }
}

fn get_texture<'a>(emote: &'a Emote, ctx: &Context) -> Option<&'a TextureHandle> {
    let frames_opt = emote.data.as_ref();
    match frames_opt {
      Some(frames) => {
        if emote.duration_msec > 0 {
          let time = chrono::Utc::now();
          let target_progress = (time.second() as u16 * 1000 + time.timestamp_subsec_millis() as u16) % emote.duration_msec;

          let mut progress_msec : u16 = 0;
          let mut result_frame: Option<&TextureHandle> = None;
          let mut next_frame_msec: Option<u16> = None;

          for (frame, msec) in frames {
            if result_frame.is_some() {
                next_frame_msec = Some(msec.to_owned());
                break;
            }

            progress_msec += msec; 

            if progress_msec >= target_progress {

              result_frame = Some(frame);
            }
          };
          
          if let Some(msec_to_next_frame) = next_frame_msec.map(|x|  progress_msec + x - target_progress).or_else(|| Some(emote.duration_msec - target_progress)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(msec_to_next_frame.into()));
          }
          
          return result_frame;
        }
        else if let Some((frame, _delay)) = frames.first() {
          Some(frame)
        }
        else {
          None
        }
      },
      None => None
    }
}

pub struct EmoteLoader {
  pub tx: async_channel::Sender<EmoteRequest>,
  pub rx: Receiver<EmoteResponse>,
  handle: Vec<JoinHandle<()>>,
  pub transparent_img: Option<TextureHandle>,
  pub red_img: Option<TextureHandle>,
  pub base_path: PathBuf,
  pub loading_emotes: HashSet<String>
}

impl Default for EmoteLoader {
  fn default() -> Self {
    Self { 
      tx: async_channel::bounded::<EmoteRequest>(10000).0,
      rx: async_channel::bounded::<EmoteResponse>(10000).1, 
      handle: Default::default(), 
      transparent_img: None,
      red_img: None,
      base_path: Default::default(), 
      loading_emotes: Default::default() 
    }
  }
}

impl EmoteLoader {
  pub fn new(app_name: &str, runtime: &Runtime) -> Self {
    let (in_tx, in_rx) = async_channel::bounded::<EmoteRequest>(10000);
    let (out_tx, out_rx) = async_channel::bounded::<EmoteResponse>(10000);
    let cache_path = cache_path_from_app_name(app_name).expect_or_log("Failed to locate an appropiate location to store cache files");

    let mut tasks : Vec<JoinHandle<()>> = Vec::new();
    for n in 1..num_cpus::get_physical() {
      let cache_path = cache_path.clone();
      let in_rx = in_rx.clone();
      let out_tx = out_tx.clone();
      let task : JoinHandle<()> = runtime.spawn(async move { 
        debug!("starting emote thread {n}");
        let client = reqwest::Client::new();
        loop {
          let recv_msg = in_rx.recv().await;
          if let Ok(msg) = recv_msg {
            let out_msg = match msg {
              EmoteRequest::ChannelEmoteImage { name, id, url, path, extension, channel_name, css_anim } => {
                let data = imaging::get_image_data(&name, &[&url], &cache_path.join(path), &id, &extension, &client, &css_anim).await;
                EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data }
              },
              EmoteRequest::ChannelBadgeImage { name, id, url, path, extension, channel_name } => {
                let data = imaging::get_image_data(&name, &[&url], &cache_path.join(path), &id, &extension, &client, &None).await;
                EmoteResponse::ChannelBadgeImageLoaded { name, channel_name, data }
              },
              EmoteRequest::GlobalEmoteImage { name, id, url, path, extension } => {
                let data = imaging::get_image_data(&name, &[&url], &cache_path.join(path), &id, &extension, &client, &None).await;
                EmoteResponse::GlobalEmoteImageLoaded { name, data }
              },
              EmoteRequest::GlobalBadgeImage { name, id, url, path, extension } => {
                let data = imaging::get_image_data(&name, &[&url], &cache_path.join(path), &id, &extension, &client, &None).await;
                EmoteResponse::GlobalBadgeImageLoaded { name, data }
              },
              EmoteRequest::TwitchMsgEmoteImage { name, id } => {
                let data = imaging::get_image_data(
                  &name, &[
                    &format!("https://static-cdn.jtvnw.net/emoticons/v2/{id}/animated/light/3.0"), 
                    &format!("https://static-cdn.jtvnw.net/emoticons/v2/{id}/static/light/3.0")
                  ], &cache_path.join("twitch/"), &id, &None, &client, &None).await;
                EmoteResponse::TwitchMsgEmoteLoaded { name, id, data }
              },
              EmoteRequest::YouTubeMsgEmoteImage { name, url, path } => {
                //info!("{n} loading youtube emote '{}'", name);
                let data = imaging::get_image_data(&name, &[&url], &cache_path.join(path), &name, &None, &client, &None).await;
                EmoteResponse::YouTubeMsgEmoteLoaded { name, data }
              },
              EmoteRequest::TwitchEmoteSetRequest { token, emote_set_id, force_redownload } => {
                let data = twitch_get_emote_set(&token, &emote_set_id, &cache_path, &client, force_redownload).await;
                EmoteResponse::TwitchEmoteSetResponse { emote_set_id, response: data }
              },
              EmoteRequest::TwitchBadgeEmoteListRequest { channel_id, channel_name, token, force_redownload } => {
                let data = load_channel_emotes(&channel_id, &token, &cache_path, &client, force_redownload);
                let badge_list = twitch_get_channel_badges(&token, &channel_id, &cache_path, &client, force_redownload);
                match out_tx.send(EmoteResponse::ChannelEmoteListResponse { channel_name: channel_name.to_owned(), response: data.await }).await {
                  Ok(()) => (),
                  Err(e) => warn!("Error sending event: {}", e)
                };
                EmoteResponse::ChannelBadgeListResponse { channel_name, response: badge_list.await }
              },
              EmoteRequest::DggFlairEmotesRequest { channel_name, cdn_base_url, force_redownload } => {
                let emote_list = dgg::load_dgg_emotes(&channel_name, &cdn_base_url, &cache_path, &client, force_redownload);
                let badge_list = dgg::load_dgg_flairs(&channel_name, &cdn_base_url, &cache_path, &client, force_redownload);
                match out_tx.send(EmoteResponse::ChannelEmoteListResponse { channel_name: channel_name.to_owned(), response: emote_list.await }).await {
                  Ok(()) => (),
                  Err(e) => warn!("Error sending event: {}", e)
                };
                EmoteResponse::ChannelBadgeListResponse { channel_name: channel_name.to_owned(), response: badge_list.await }
              },
              EmoteRequest::GlobalEmoteListRequest { force_redownload } => {
                let data = load_global_emotes(&cache_path, &client, force_redownload).await;
                EmoteResponse::GlobalEmoteListResponse { response: data }
              },
              EmoteRequest::TwitchGlobalBadgeListRequest { token, force_redownload } => {
                let data = twitch_get_global_badges(&token, &cache_path, &client, force_redownload).await;
                EmoteResponse::TwitchGlobalBadgeListResponse { response: data }
              }
            };
            match out_tx.send(out_msg).await {
              Ok(()) => (),
              Err(e) => warn!("Error sending event: {}", e)
            };
          }
          // everything ends up handled by one thread without this delay
          //tokio::time::sleep(Duration::from_millis(10)).await;
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
      red_img: None,
      base_path: cache_path,
      loading_emotes: Default::default()
     }
  }

  pub fn request_emote(&mut self, name: &String, request: EmoteRequest) {
    if self.loading_emotes.insert(name.to_owned()) {
      if let Err(e) = self.tx.try_send(request) {
        info!("Error sending emote load request: {}", e);
      }
    }
  }

  pub fn close(&self) {
    self.handle.iter().for_each(|x| x.abort());
  }  
}

pub async fn load_channel_emotes(
  channel_id: &String,
  token: &String,
  cache_path: &Path,
  client: &reqwest::Client,
  force_redownload: bool
) -> std::result::Result<HashMap<String, Emote>, anyhow::Error> {
  let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{channel_id}");
  let ffz_emotes = process_emote_json(
    &ffz_url,
    cache_path,
    &format!("ffz-channel-json-{channel_id}"),
    None,
    client,
    force_redownload
  ).await?;
  let bttv_url = format!("https://api.betterttv.net/3/cached/users/twitch/{channel_id}");
  let bttv_emotes = process_emote_json(
    &bttv_url,
    cache_path,
    &format!("bttv-channel-json-{channel_id}"),
    None,
    client,
    force_redownload
  ).await?;
  //let seventv_url = format!("https://api.7tv.app/v2/users/{channel_id}/emotes");
  let seventv_url = format!("https://7tv.io/v3/users/twitch/{channel_id}");
  let seventv_emotes = process_emote_json(
    &seventv_url,
    cache_path,
    &format!("7tv-channel-json-{channel_id}"),
    None,
    client,
    force_redownload
  ).await?;
  let twitch_url = format!("https://api.twitch.tv/helix/chat/emotes?broadcaster_id={channel_id}");
  let twitch_follower_emotes = process_twitch_follower_emote_json(
    &twitch_url,
    cache_path,
    &format!("twitch-{channel_id}"),
    Some([
      ("Authorization", &format!("Bearer {token}")),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())].to_vec()
    ),
    client,
    force_redownload
  ).await?;

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

pub async fn load_global_emotes(
  cache_path: &Path,
  client: &reqwest::Client,
  force_redownload: bool
) -> std::result::Result<HashMap<String, Emote>, anyhow::Error> {
  let bttv_emotes = process_emote_json(
    "https://api.betterttv.net/3/cached/emotes/global",
    cache_path,
    "bttv-global-json",
    None,
    client,
    force_redownload
  ).await?;
  let seventv_emotes = process_emote_json(
    "https://7tv.io/v3/emote-sets/62cdd34e72a832540de95857",
    cache_path,
    "7tv-global-json",
    None,
    client,
    force_redownload
  ).await?;

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
async fn process_emote_json(url: &str, cache_path: &Path, path: &str, headers: Option<Vec<(&str, &String)>>, client: &reqwest::Client, force_redownload: bool) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  fetch::process_emote_json(url, cache_path.join(path).to_str().unwrap_or_log(), headers, client, force_redownload).await
}

async fn process_twitch_follower_emote_json(twitch_url: &str, cache_path: &Path, path: &str, headers: Option<Vec<(&str, &String)>>, client: &reqwest::Client, force_redownload: bool) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  fetch::process_twitch_follower_emote_json(twitch_url, cache_path.join(path).to_str().unwrap_or_log(), headers, client, force_redownload).await
}

async fn process_badge_json(room_id: &str, url: &str, cache_path: &Path, path: &str, headers: Option<Vec<(&str, &String)>>, client: &reqwest::Client, force_redownload: bool) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  fetch::process_badge_json(room_id, url, cache_path.join(path).to_str().unwrap_or_log(), headers, client, force_redownload).await
}

pub async fn twitch_get_emote_set(token : &String, emote_set_id : &String, cache_path: &Path, client: &reqwest::Client, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> { 
  if emote_set_id.contains(':') || emote_set_id.contains('-') || emote_set_id.contains("emotesv2") {
    return Ok(Default::default());
  }

  let emotes = process_emote_json(
    &format!("https://api.twitch.tv/helix/chat/emotes/set?emote_set_id={emote_set_id}"),
    cache_path,
    &format!("twitch-emote-set-{emote_set_id}"),
    Some([
      ("Authorization", &format!("Bearer {token}")),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
    ].to_vec()),
    client,
    force_redownload
  ).await;

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

pub async fn twitch_get_global_badges(token : &String, cache_path: &Path, client: &reqwest::Client, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> { 
  let emotes = process_badge_json(
    "global",
    "https://api.twitch.tv/helix/chat/badges/global",
    cache_path,
    "twitch-badges-global",
    Some([
      ("Authorization", &format!("Bearer {token}")),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
    ].to_vec()),
    client,
    force_redownload
  ).await;

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

pub async fn twitch_get_channel_badges(token : &String, room_id : &String, cache_path: &Path, client: &reqwest::Client, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> { 
  let emotes = process_badge_json(
    room_id,
    &format!("https://api.twitch.tv/helix/chat/badges?broadcaster_id={room_id}"),
    cache_path,
    &format!("twitch-badges-channel-{room_id}"),
    Some([
      ("Authorization", &format!("Bearer {token}")),
      ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())
    ].to_vec()),
    client,
    force_redownload
  ).await;

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

fn load_emote_data(emote: &mut Emote, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>) {
  if emote.data.is_none() {
    emote.data = imaging::load_to_texture_handles(ctx, data);
    emote.duration_msec = match emote.data.as_ref() {
      Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
      _ => 0,
    };
  }
  emote.loaded = EmoteStatus::Loaded;
  loading_emotes.remove(&emote.name);
  ctx.request_repaint();
}

pub trait LoadEmote {
  fn update_emote(&mut self, emote_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>);
  fn update_badge(&mut self, badge_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>);
}

impl LoadEmote for ChannelShared {
  fn update_emote(&mut self, emote_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>) {
    if let Some(transient) = self.transient.as_mut() && let Some(emotes) = transient.channel_emotes.as_mut() && let Some(emote) = emotes.get_mut(emote_name) {
      load_emote_data(emote, ctx, data, loading_emotes);
    }
  }
  fn update_badge(&mut self, badge_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>) {
    if let Some(transient) = self.transient.as_mut() && let Some(badges) = transient.badge_emotes.as_mut() && let Some(emote) = badges.get_mut(badge_name) {
      load_emote_data(emote, ctx, data, loading_emotes);
    }
  }
}

impl TemplateApp {
  pub fn update_emote(&mut self, emote_name: &String, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>) {
    if let Some(emote) = self.global_emotes.get_mut(emote_name) {
      load_emote_data(emote, ctx, data, &mut self.emote_loader.loading_emotes)
    }
  }
}

impl LoadEmote for Provider {
  fn update_emote(&mut self, emote_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>) {
    if let Some(emote) = self.emotes.get_mut(emote_name) {
      load_emote_data(emote, ctx, data, loading_emotes)
    }
  }
  fn update_badge(&mut self, badge_name: &str, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashSet<String>) {
    if let Some(global_badges) = &mut self.global_badges && let Some(emote) = global_badges.get_mut(badge_name) {
      load_emote_data(emote, ctx, data, loading_emotes)
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
      Ok(mut emotes) => {
        for (_, emote) in emotes.iter_mut() {
          emote.source = EmoteSource::Channel;
          emote.channel_name = self.channel_name().to_owned();
        }
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
      Ok(mut badges) => {
        for (_, badge) in badges.iter_mut() {
          badge.source = EmoteSource::ChannelBadge;
          badge.channel_name = self.channel_name().to_owned();
        }
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