use std::collections::HashMap;

/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
 
use chrono::{Utc, NaiveDateTime, DateTime};
use curl::easy::{Easy};
use futures::StreamExt;
use itertools::Itertools;
use regex::Regex;
use tokio::{runtime::Runtime, sync::mpsc};
use tokio_tungstenite::{tungstenite::{http::{header::COOKIE}, client::IntoClientRequest}, connect_async_tls_with_config};

use crate::emotes::{fetch, Emote, EmoteLoader, CssAnimationData};

use super::{IncomingMessage, Channel, OutgoingMessage, ProviderName, ChatMessage, UserProfile, ChannelTransient, make_request, ChatManager, convert_color_hex};

const DGG_CHANNEL_NAME : &str = "Destiny";

pub fn init_channel<'a>() -> Channel {
  let channel = Channel {  
    provider: ProviderName::DGG,  
    channel_name: DGG_CHANNEL_NAME.to_owned(),
    roomid: Default::default(),
    send_history: Default::default(),
    send_history_ix: None,
    transient: None
  };
  channel
}

pub fn open_channel<'a>(user_name: &String, token: &String, channel: &mut Channel, runtime: &Runtime, emote_loader: &EmoteLoader) -> ChatManager {
  let (out_tx, out_rx) = mpsc::channel::<IncomingMessage>(32);
  let (in_tx, in_rx) = mpsc::channel::<OutgoingMessage>(32);
  let token2 = token.to_owned();
  let name2 = user_name.to_owned();

  let handle = runtime.spawn(async move { 
    spawn_websocket_client(name2, token2, out_tx, in_rx).await
  });

  channel.transient = Some(ChannelTransient {
    channel_emotes: load_dgg_emotes(emote_loader),
    badge_emotes: load_dgg_flairs(emote_loader),
    status: None
  });

  ChatManager {
    handle: handle,
    in_tx,
    out_rx,
  }
}

impl ChatManager {
  pub fn close(&mut self) {
    self.in_tx.try_send(OutgoingMessage::Quit {}).expect("channel failure");
    std::thread::sleep(std::time::Duration::from_millis(1000));
    self.handle.abort();
  }
}

async fn spawn_websocket_client(_user_name : String, token: String, tx : mpsc::Sender<IncomingMessage>, mut rx: mpsc::Receiver<OutgoingMessage>) {
  let mut quitted = false;

  let cookie = format!("authtoken={}", token);
  let mut request = "wss://chat.destiny.gg/ws".into_client_request().expect("failed to build request");
  request.headers_mut().append(COOKIE, cookie.parse().unwrap());

  let (mut socket, _) = connect_async_tls_with_config(request, None, None).await.expect("failed to connect to wss");

  while !quitted {
    tokio::select! {
      Some(result) = socket.next()  => {
        match result {
          Ok(message) => {
            println!("{}", message);
            if let Ok(message) = message.into_text() 
              && let Some((command, msg)) = message.split_once(" ")
              && let Ok(msg) = serde_json::from_str::<Msg>(&msg) {
                match command {
                  "MSG" => {
                    let features = msg.features.iter().filter_map(|f| if f != &"subscriber" { Some(f.to_owned()) } else { None }).collect_vec();
                    let cmsg = ChatMessage { 
                      provider: ProviderName::DGG,
                      channel: DGG_CHANNEL_NAME.to_owned(),
                      username: msg.nick, 
                      timestamp: DateTime::from_utc(NaiveDateTime::from_timestamp(msg.timestamp as i64, 0), Utc), 
                      message: msg.data,
                      profile: UserProfile { 
                        badges: if features.len() > 0 { Some(features) } else { None },
                        display_name: None, 
                        color: None
                      },
                      ..Default::default()
                    };
                    match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
                      Ok(_) => (),
                      Err(x) => println!("Send failure: {}", x)
                    };
                  },
                  "JOIN" => (),
                  "QUIT" => (),
                  _ => println!("unknown dgg command: {command}")
                }
            } 
          },
          Err(e) => println!("IRC Stream error: {:?}", e)
        }
      },
      Some(out_msg) = rx.recv() => {
        match out_msg {
          OutgoingMessage::Chat { channel_name : _, message : _ } => { 

          },
          OutgoingMessage::Leave { channel_name : _ } => {
            socket.close(None).await.expect("Error while quitting IRC server"); quitted = true;
          },
          _ => ()
        };
      }
    };
  }
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

  println!("{}", &authorize_url);

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
        Err(e) => { println!("error parsing dgg auth response: {}", e); None }
      }
    },
    Err(e) => { println!("error getting dgg auth token: {}", e); None }
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
        Err(e) => { println!("error parsing dgg auth response: {}", e); None }
      }
    },
    Err(e) => { println!("error getting dgg auth token: {}", e); None }
  }
}

pub fn load_dgg_flairs(emote_loader: &EmoteLoader) -> Option<HashMap<String, Emote>> {
  let json_path = &emote_loader.base_path.join("cache/dgg-flairs.json");
  let json = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/flairs/flairs.json", json_path.to_str(), None).expect("failed to download flair json");
  let emotes = serde_json::from_str::<Vec<DggFlair>>(&json).expect("failed to load flair json");
    let mut result : HashMap<String, Emote> = Default::default();
    for emote in emotes {
      let image = &emote.image.first().unwrap();
      let (id, extension) = image.name.split_once(".").unwrap();

      result.insert(emote.name.to_owned(), Emote { 
        name: emote.name, 
        display_name: Some(emote.label),
        color: convert_color_hex(Some(&emote.color)),
        id: id.to_owned(), 
        data: None, 
        loaded: crate::emotes::EmoteStatus::NotLoaded, 
        duration_msec: 0, 
        url: image.url.to_owned(), 
        path: "cache/dgg/".to_owned(), 
        extension: Some(extension.to_owned()), 
        zero_width: false,
        css_anim: None });
    }
    Some(result)
}

pub fn load_dgg_emotes(emote_loader: &EmoteLoader) -> Option<HashMap<String, Emote>> {
  let json_path = &emote_loader.base_path.join("cache/dgg-emotes.json");
  let css_path = &emote_loader.base_path.join("cache/dgg-emotes.css");
  let json = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.json", json_path.to_str(), None).expect("failed to download emote json");
  let css = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", css_path.to_str(), None).expect("failed to download emote css");
  let emotes = serde_json::from_str::<Vec<DggEmote>>(&json).expect("failed to load emote json");
    let mut result : HashMap<String, Emote> = Default::default();
    for emote in emotes {
      let image = &emote.image.first().unwrap();
      let (id, extension) = image.name.split_once(".").unwrap();

      let prefix = &emote.prefix;
      let regex = Regex::new(&format!("\\.emote\\.{prefix}\\s*\\{{\\s*width:\\s*(.*?)px; .*? animation: {prefix}\\-anim (.*?)(ms|s) steps\\((.*?)\\) (.*?)(?:;|\\s)")).unwrap();
      let css_anim = match regex.captures(&css) {
        Some(captures) => {
          let width = captures.get(1);
          let time = captures.get(2);
          let time_unit = captures.get(3);
          let steps = captures.get(4);
          let loops = captures.get(5);
          if let Some(width) = width.and_then(|x| x.as_str().parse::<u32>().ok()) 
              && let Some(time) = time.and_then(|x| x.as_str().parse::<f32>().ok()) 
              && let Some(time_unit) = time_unit.and_then(|x| Some(x.as_str())) 
              && let Some(steps) = steps.and_then(|x| x.as_str().parse::<isize>().ok())
              && let Some(loops) = loops.and_then(|x| x.as_str().parse::<isize>().ok()) {
            Some(CssAnimationData {
              width: width,
              cycle_time_msec: match time_unit { "ms" => time as isize, _ => (time * 1000.) as isize },
              steps: steps,
              loops: loops
            })
          } else {
            None
          }
        },
        None => None
      };

      result.insert(emote.prefix.to_owned(), Emote { 
        name: emote.prefix, 
        id: id.to_owned(), 
        data: None, 
        color: None,
        loaded: crate::emotes::EmoteStatus::NotLoaded, 
        duration_msec: 0, 
        url: image.url.to_owned(), 
        path: "cache/dgg/".to_owned(), 
        extension: Some(extension.to_owned()), 
        zero_width: false,
        css_anim: css_anim,
        display_name: None });
    }
    Some(result)
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
struct Msg {
  nick: String,
  features: Vec<String>,
  timestamp: usize,
  data: String
}

#[derive(serde::Deserialize)]
struct DggEmote {
  prefix: String,
  image: Vec<DggEmoteImage>
}

#[derive(serde::Deserialize)]
struct DggEmoteImage {
  url: String,
  name: String
}

#[derive(serde::Deserialize)]
struct DggFlair {
  label: String,
  name: String,
  hidden: bool,
  priority: isize,
  color: String,
  image: Vec<DggEmoteImage>
}