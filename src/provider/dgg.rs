/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::HashMap, path::{PathBuf, Path}};
use async_channel::{Sender, Receiver};
use chrono::{Utc, NaiveDateTime, DateTime};
use curl::easy::{Easy};
use futures::{StreamExt, SinkExt};
use itertools::Itertools;
use tracing::{trace, info,warn,error};
use crate::{provider::MessageType, emotes::EmoteRequest};
use regex::Regex;
use tokio::{runtime::Runtime};
use tokio_tungstenite::{tungstenite::{http::{header::COOKIE}, client::IntoClientRequest, Message}, connect_async_tls_with_config};
use crate::{emotes::{fetch, Emote, EmoteLoader, CssAnimationData}, provider::ChannelStatus};
use super::{IncomingMessage, Channel, OutgoingMessage, ProviderName, ChatMessage, UserProfile, ChannelTransient, make_request, ChatManager, convert_color_hex};
use tracing_unwrap::{OptionExt, ResultExt};

pub const DGG_CHANNEL_NAME : &str = "Destiny";

pub fn init_channel() -> Channel {
  Channel {  
    provider: ProviderName::DGG,  
    channel_name: DGG_CHANNEL_NAME.to_owned(),
    show_in_mentions_tab: true,
    roomid: Default::default(),
    send_history: Default::default(),
    send_history_ix: None,
    transient: None,
    users: Default::default(),
    dgg_cdn_url: "https://cdn.destiny.gg/2.42.0/".to_owned(),
    dgg_status_url: "wss://live.destiny.gg/ws".to_owned(),
    dgg_chat_url: "wss://chat.destiny.gg/ws".to_owned(),
  }
}

pub fn open_channel(user_name: &String, token: &String, channel: &mut Channel, runtime: &Runtime, emote_loader: &EmoteLoader) -> ChatManager {
  let (mut out_tx, out_rx) = async_channel::unbounded::<IncomingMessage>();
  let (in_tx, mut in_rx) = async_channel::unbounded::<OutgoingMessage>();

  let status_url = channel.dgg_status_url.to_owned();
  let chat_url = channel.dgg_chat_url.to_owned();

  let mut out_tx_2 = out_tx.clone();
  let handle2 = runtime.spawn(async move { 
    loop {
      if spawn_websocket_live_client(&status_url, &mut out_tx_2).await {
        break;
      }
    }
  });

  let name1 = user_name.to_owned();
  let token1 = token.to_owned();
  let handle = runtime.spawn(async move { 
    loop {
      if spawn_websocket_chat_client(&chat_url, &name1, &token1, &mut out_tx, &mut in_rx).await {
        break;
      }
    }
  });

  channel.transient = Some(ChannelTransient {
    channel_emotes: None,
    badge_emotes: None,
    status: None
  });

  match emote_loader.tx.try_send(EmoteRequest::DggFlairEmotesRequest { 
    channel_name: channel.channel_name.to_owned(), 
    cdn_base_url: channel.dgg_cdn_url.to_owned(), 
    force_redownload: false 
  }) {  
    Ok(_) => {},
    Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
  };

  let handles = vec![ handle, handle2 ];

  ChatManager {
    username: user_name.to_owned(),
    handles,
    in_tx,
    out_rx,
  }
}

impl ChatManager {
  pub fn close(&mut self) {
    self.in_tx.try_send(OutgoingMessage::Quit {}).expect_or_log("channel failure");
    std::thread::sleep(std::time::Duration::from_millis(500));
    for handle in &self.handles {
      handle.abort();
    }
  }
}

async fn spawn_websocket_live_client(dgg_status_url: &String, tx : &mut Sender<IncomingMessage>) -> bool {
  let request = dgg_status_url.into_client_request().expect_or_log("failed to build request");
  let (mut socket, _) = connect_async_tls_with_config(request, None, None).await.expect_or_log("failed to connect to wss");

  loop {
    tokio::select! {
      Some(result) = socket.next() => {
        match result {
          Ok(message) => {
            if let Ok(message) = message.into_text().inspect_err(|f| warn!("websocket error: {}", f)) 
              && message.contains("dggApi:streamInfo")
              && let Ok(msg) = serde_json::from_str::<DggApiMsg>(&message).inspect_err(|f| warn!("json parse error: {}\n {}", f, message))
              && msg.r#type == Some("dggApi:streamInfo".to_string())
              && let Some(data) = msg.data
              && let Some(streams) = data.streams
              && let Some(yt_data) = streams.youtube {
                let status_msg = IncomingMessage::StreamingStatus { channel: DGG_CHANNEL_NAME.to_owned(), status: Some(ChannelStatus { 
                  game_name: yt_data.game, 
                  is_live: yt_data.live.unwrap_or(false), 
                  title: yt_data.status_text,  
                  viewer_count: yt_data.viewers, 
                  started_at: yt_data.started_at 
                }) };

                if let Err(e) = tx.try_send(status_msg) { warn!("error sending dgg stream status: {}", e) }
            }
          },
          Err(e) => {
            error!("Websocket error: {:?}", e);
            return false;
          }
        }
      }
    };
  }
}

async fn spawn_websocket_chat_client(dgg_chat_url: &String, _user_name : &String, token: &String, tx : &mut Sender<IncomingMessage>, rx: &mut Receiver<OutgoingMessage>) -> bool {
  let mut quitted = false;

  let cookie = format!("authtoken={}", token);
  let mut request = dgg_chat_url.into_client_request().expect_or_log("failed to build request");
  request.headers_mut().append(COOKIE, cookie.parse().unwrap_or_log());
  //info!("adding cookie {} {}", cookie, r);
  //for item in request.headers().iter() {
  //  info!("{}: {:?}", item.0, item.1);
  //}

  let (mut socket, _) = connect_async_tls_with_config(request, None, None).await.expect_or_log("failed to connect to wss");
  //let (mut write, mut read) = socket.split();

  while !quitted {
    tokio::select! {
      Some(result) = socket.next() => {
        match result {
          Ok(message) => {
            if message.is_ping() {
              trace!("Received Ping: {:?}", message);
              socket.send(Message::Pong(message.into_data())).await
              .inspect_err(|f| info!("socket send Pong error: {}", f))
              .expect_or_log("Error sending websocket Pong message");
            }
            else if !message.is_text() {
              warn!("{:?}", message);
            }
            else if message.is_text() && let Ok(message) = message.into_text().inspect_err(|f| info!("websocket error: {}", f)) 
              && let Some((command, msg)) = message.split_once(' ') {
                match command {
                  "MSG" => {
                    if let Ok(msg) = serde_json::from_str::<MsgMessage>(msg).inspect_err(|f| info!("json parse error: {}\n {}", f, message)) {
                      let features = msg.features.iter().filter_map(|f| if f != "subscriber" { Some(f.to_owned()) } else { None }).collect_vec();
                      let cmsg = ChatMessage { 
                        provider: ProviderName::DGG,
                        channel: DGG_CHANNEL_NAME.to_owned(),
                        username: msg.nick.to_lowercase(), 
                        timestamp: DateTime::from_utc(NaiveDateTime::from_timestamp(msg.timestamp as i64 / 1000, (msg.timestamp % 1000 * 1000_usize.pow(2)) as u32 ), Utc), 
                        message: msg.data.unwrap_or_log(),
                        profile: UserProfile { 
                          badges: if !features.is_empty() { Some(features) } else { None },
                          display_name: Some(msg.nick), 
                          color: None
                        },
                        ..Default::default()
                      };
                      match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
                        Ok(_) => (),
                        Err(x) => info!("Send failure for MSG: {}", x)
                      };
                    }
                  },
                  "BROADCAST" => {
                    if let Ok(msg) = serde_json::from_str::<BroadcastMessage>(msg).inspect_err(|f| info!("json parse error: {}\n {}", f, message)) {
                      let cmsg = ChatMessage { 
                        provider: ProviderName::DGG,
                        channel: DGG_CHANNEL_NAME.to_owned(),
                        timestamp: DateTime::from_utc(NaiveDateTime::from_timestamp(msg.timestamp as i64 / 1000, (msg.timestamp % 1000 * 1000_usize.pow(2)) as u32 ), Utc), 
                        message: msg.data.unwrap_or_log(),
                        msg_type: MessageType::Announcement,
                        ..Default::default()
                      };
                      match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
                        Ok(_) => (),
                        Err(x) => info!("Send failure for MSG: {}", x)
                      };
                    }
                  },
                  "REFRESH" => {
                    // REFRESH {\"nick\":\"Bob\",\"features\":[\"subscriber\",\"flair1\"],\"timestamp\":1660506127552}
                  },
                  "JOIN" => {
                    if let Ok(msg) = serde_json::from_str::<MsgMessage>(msg).inspect_err(|f| info!("json parse error: {}\n {}", f, message)) {
                      match tx.try_send(IncomingMessage::UserJoin { channel: DGG_CHANNEL_NAME.to_owned(), username: msg.nick.to_owned(), display_name: msg.nick }) {
                        Ok(_) => (),
                        Err(x) => info!("Send failure for JOIN: {}", x)
                      };
                    }
                  },
                  "QUIT" => {
                    if let Ok(msg) = serde_json::from_str::<MsgMessage>(msg).inspect_err(|f| info!("json parse error: {}\n {}", f, message)) {
                      match tx.try_send(IncomingMessage::UserLeave { channel: DGG_CHANNEL_NAME.to_owned(), username: msg.nick.to_owned(), display_name: msg.nick }) {
                        Ok(_) => (),
                        Err(x) => info!("Send failure for QUIT: {}", x)
                      };
                    }
                  },
                  "NAMES" => {
                    if let Ok(msg) = serde_json::from_str::<NamesMessage>(msg).inspect_err(|f| info!("json parse error: {}\n {}", f, message)) {
                      for user in msg.users {
                        match tx.try_send(IncomingMessage::UserJoin { channel: DGG_CHANNEL_NAME.to_owned(), username: user.nick.to_owned(), display_name: user.nick }) {
                          Ok(_) => (),
                          Err(x) => info!("Send failure for NAMES: {}", x)
                        };
                      }
                    }
                  },
                  "ERR" => {
                    if let Ok(msg) = serde_json::from_str::<DggErr>(msg).inspect_err(|f| info!("json parse error: {}\n {}", f, message)) {
                      
                      match tx.try_send(IncomingMessage::PrivMsg { message: ChatMessage {
                        channel: DGG_CHANNEL_NAME.to_owned(), 
                        provider: ProviderName::DGG, 
                        message: match msg.description.as_str() {
                          "duplicate" => "The message is identical to the last one you sent".to_owned(),
                          _ => msg.description.to_owned()
                        },
                        msg_type: MessageType::Error,
                        ..Default::default() 
                      } }) {
                        Ok(_) => (),
                        Err(x) => info!("Send failure for ERR: {}", x)
                      };
                    }
                  },
                  _ => warn!("unknown dgg command: {:?}", message)
                }
            }
          },
          Err(e) => {
            error!("Websocket error: {:?}", e);
            return false;
          }
        }
      },
      Ok(out_msg) = rx.recv() => {
        match out_msg {
          OutgoingMessage::Chat { channel : _, message } => { 
            socket.send(Message::Text(format!("MSG {{\"data\":\"{}\"}}\r", message))).await
              .inspect_err(|f| info!("socket send error: {}", f))
              .expect_or_log("Error sending websocket message");
          },
          OutgoingMessage::Leave { channel_name : _ } => {
            socket.close(None).await.expect_or_log("Error while quitting IRC server"); quitted = true;
          },
          _ => ()
        };
      }
    };
  }
  true
}

const REDIRECT_URI : &str = "https://dbckr.github.io/GigachatAuth";
const CLIENT_ID : &str = "dbrq5gUQDWmv6jBzFt9UwpN8VQOIeO7i";

pub fn begin_authenticate(ctx: &egui::Context) -> String {
  let secret = sha256::digest("S0eHxQsXfbo!l=Pk~pf7[ZWSC.C7BlWK1YFNgKkqxQ!ojZ1C~tYyVh3+SsxCn-kY");
  let code_verifier = format!("{:x}{:x}", rand::random::<u128>(), rand::random::<u128>());
  let code_challenge = base64::encode(sha256::digest(format!("{}{}", code_verifier, secret)));

  let state = format!("{}", rand::random::<u128>());
  let authorize_url = format!("https://www.destiny.gg/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}", 
    CLIENT_ID, 
    REDIRECT_URI, 
    state,
    code_challenge);

  info!("{}", &authorize_url);

  ctx.output().open_url(&authorize_url);
  code_verifier
}

pub fn complete_authenticate(code: &str, code_verifier: &String) -> Option<String> {
  let mut easy = Easy::new();
  let url = format!("https://www.destiny.gg/oauth/token?grant_type=authorization_code&code={}&client_id={}&redirect_uri={}&code_verifier={}",
    code,
    CLIENT_ID,
    REDIRECT_URI,
    code_verifier);

  match make_request(&url, None, &mut easy) {
    Ok(resp) => {
      let result: Result<AuthResponse, _> = serde_json::from_str(&resp);
      match result {
        Ok(r) => Some(r.access_token),
        Err(e) => { info!("error parsing dgg auth response: {}", e); None }
      }
    },
    Err(e) => { info!("error getting dgg auth token: {}", e); None }
  }
}

pub fn refresh_auth_token(refresh_token: String) -> Option<String> {
  let mut easy = Easy::new();
  let url = format!("https://www.destiny.gg/oauth/token?grant_type=refresh_token&client_id={}&refresh_token={}",
    CLIENT_ID,
    refresh_token);

  match make_request(&url, None, &mut easy) {
    Ok(resp) => {
      let result: Result<AuthResponse, _> = serde_json::from_str(&resp);
      match result {
        Ok(r) => Some(r.access_token),
        Err(e) => { info!("error parsing dgg auth response: {}", e); None }
      }
    },
    Err(e) => { info!("error getting dgg auth token: {}", e); None }
  }
}

pub fn load_dgg_flairs(cdn_base_url: &str, cache_path: &Path, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> {
  let json_path = &cache_path.join("dgg-flairs.json");
  let json = fetch::get_json_from_url(format!("{}/flairs/flairs.json", cdn_base_url.trim_end_matches('/')).as_str(), json_path.to_str(), None, force_redownload)?;
  let emotes = serde_json::from_str::<Vec<DggFlair>>(&json)?;
  let mut result : HashMap<String, Emote> = Default::default();
  for emote in emotes {
    let image = &emote.image.first().unwrap_or_log();
    let (id, extension) = image.name.split_once('.').unwrap_or_log();

    result.insert(emote.name.to_owned(), Emote { 
      name: emote.name, 
      display_name: Some(emote.label),
      color: convert_color_hex(Some(&emote.color)),
      id: id.to_owned(), 
      data: None, 
      loaded: crate::emotes::EmoteStatus::NotLoaded, 
      duration_msec: 0, 
      url: image.url.to_owned(), 
      path: "dgg/".to_owned(), 
      extension: Some(extension.to_owned()), 
      zero_width: false,
      css_anim: None,
      priority: emote.priority,
      hidden: emote.hidden,
      texture_expiration: None });
  }
  Ok(result)
}

pub fn load_dgg_emotes(cdn_base_url: &String, cache_path: &PathBuf, force_redownload: bool) -> Result<HashMap<String, Emote>, anyhow::Error> {
  let css_path = &cache_path.join("dgg-emotes.css");
  let css = fetch::get_json_from_url(format!("{}/emotes/emotes.css", cdn_base_url.trim_end_matches('/')).as_str(), css_path.to_str(), None, force_redownload)?;
  let css_anim_data = CSSLoader::default().get_css_anim_data(&css);

  let json_path = &cache_path.join("dgg-emotes.json");
  let json = fetch::get_json_from_url(format!("{}/emotes/emotes.json", cdn_base_url.trim_end_matches('/')).as_str(), json_path.to_str(), None, force_redownload)?;
  let emotes = serde_json::from_str::<Vec<DggEmote>>(&json)?;
  let mut result : HashMap<String, Emote> = Default::default();
  for emote in emotes {
    let image = &emote.image.first().unwrap_or_log();
    let (id, extension) = image.name.split_once('.').unwrap_or_log();

    let prefix = &emote.prefix;
    let css_anim = css_anim_data.get(prefix);
    //info!("{} {:?}", prefix, css_anim);

    result.insert(emote.prefix.to_owned(), Emote { 
      name: emote.prefix, 
      id: id.to_owned(), 
      data: None, 
      color: None,
      loaded: crate::emotes::EmoteStatus::NotLoaded, 
      duration_msec: 0, 
      url: image.url.to_owned(), 
      path: "dgg/".to_owned(), 
      extension: Some(extension.to_owned()), 
      zero_width: false,
      css_anim: css_anim.map(|x| x.to_owned()),
      display_name: None,
      priority: 0,
      hidden: false,
      texture_expiration: None });
  }
  Ok(result)
}

pub struct CSSLoader {
  time_regex: Regex,
  steps_regex: Regex,
}

impl Default for CSSLoader {
  fn default() -> Self {
    Self { 
      time_regex: Regex::new("([\\d\\.]*?)(ms|s)").unwrap_or_log(), 
      steps_regex: Regex::new("steps\\((.*?)\\)").unwrap_or_log() 
    }
  }
}

impl CSSLoader {
  pub fn get_css_anim_data(&self, css: &str) -> HashMap<String, CssAnimationData> {
    let mut result : HashMap<String, CssAnimationData> = Default::default();
    let regex = Regex::new(r"(?s)\.emote\.([^:\-\s]*?)\s?\{[^\}]*? width: (\d+?)px;[^\}]*?animation: (?:[^\s]*?) ([^\}]*?;)").unwrap_or_log();
    let css_notabs = css.replace("\t", "  ");
    let caps = regex.captures_iter(css_notabs.as_str());
    let mut x = 0;
    for captures in caps {
      x += 1;
      println!("{:?}", captures);
      let prefix = captures.get(1).map(|x| x.as_str());
      //println!("{:?}", prefix);
      let width = captures.get(2).and_then(|x| x.as_str().parse::<u32>().ok());
      let anim = captures.get(3).map(|x| x.as_str());
      let steps = anim.and_then(|x| self.steps_regex.captures(x).and_then(|y| y.get(1)).and_then(|z| z.as_str().parse::<isize>().ok()));

      let caps = anim.and_then(|x| self.time_regex.captures(x)).unwrap_or_log();
      let time = caps.get(1).and_then(|x| x.as_str().parse::<f32>().ok());
      let unit = caps.get(2).map(|x| x.as_str());
      //info!("{:?} {:?} {:?} {:?} {:?}", width, anim, steps, time, unit);
      let time_msec = if let Some(unit) = unit && let Some(time) = time {
        match unit { "ms" => time as isize, _ => (time * 1000.) as isize }
      } else if let Some(steps) = steps {
        steps * 30
      } else {
        1000
      };

      if let Some(prefix) = prefix && let Some(width) = width && let Some(steps) = steps {
        result.insert(prefix.to_owned(), CssAnimationData {
          width,
          cycle_time_msec: time_msec,
          steps
        });
      }
    }
    println!("{}", x);
    result
  }
}

#[derive(serde::Deserialize)]
struct DggApiMsg {
  r#type: Option<String>,
  data: Option<LiveSocketMsg>
}

#[derive(serde::Deserialize)]
struct LiveSocketMsg {
  streams: Option<LiveSocketMsgStreams>
}

#[derive(serde::Deserialize)]
struct LiveSocketMsgStreams {
  //twitch: Option<LiveSocketMsgStreamDetail>,
  youtube: Option<LiveSocketMsgStreamDetail>
}

#[derive(serde::Deserialize)]
struct LiveSocketMsgStreamDetail {
  live: Option<bool>,
  game: Option<String>,
  //preview: Option<String>,
  status_text: Option<String>,
  started_at: Option<String>,
  //ended_at: Option<String>,
  //duration: Option<usize>,
  viewers: Option<usize>,
  //id: Option<String>,
  //platform: Option<String>,
  //r#type: Option<String>
}

#[derive(serde::Deserialize)]
struct AuthResponse {
  access_token: String,
  //refresh_token: String,
  //expires_in: isize,
  //scope: String,
  //token_type: String
}

#[derive(serde::Deserialize)]
struct NamesMessage {
  users: Vec<PartialMsgMessage>
}

#[derive(serde::Deserialize)]
struct PartialMsgMessage {
  nick: String
}

#[derive(serde::Deserialize)]
struct MsgMessage {
  nick: String,
  features: Vec<String>,
  timestamp: usize,
  data: Option<String>
}

#[derive(serde::Deserialize)]
struct BroadcastMessage {
  timestamp: usize,
  data: Option<String>
}

#[derive(serde::Deserialize)]
struct DggEmote {
  prefix: String,
  image: Vec<DggEmoteImage>
}

#[derive(Debug)]
#[derive(serde::Deserialize)]
pub struct DggEmoteImage {
  url: String,
  name: String
}

#[derive(serde::Deserialize)]
struct DggErr {
  description: String
}

#[derive(Debug)]
#[derive(serde::Deserialize)]
pub struct DggFlair {
  label: String,
  name: String,
  hidden: bool,
  priority: isize,
  color: String,
  image: Vec<DggEmoteImage>
}