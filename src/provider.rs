/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use async_channel::{Sender, Receiver};
use tracing::info;
use std::collections::{HashMap, VecDeque, HashSet};

use chrono::{DateTime, Utc};
use curl::easy::Easy;
use crate::error_util::{LogErrResult};

use crate::emotes::{Emote};

pub mod twitch;
//pub mod youtube;
pub mod dgg;

#[derive(Clone)]
pub enum IncomingMessage {
  PrivMsg { message: ChatMessage },
  EmoteSets { provider: ProviderName, emote_sets: Vec<String> },
  MsgEmotes { provider: ProviderName, emote_ids: Vec<(String, String)> },
  RoomId { channel: String, room_id: String },
  StreamingStatus { channel: String, status: Option<ChannelStatus>},
  UserJoin { channel: String, username: String },
  UserLeave { channel: String, username: String }
}

impl Default for IncomingMessage {
    fn default() -> Self {
        Self::PrivMsg { message: Default::default() }
    }
}

#[derive(Debug)]
pub enum OutgoingMessage {
  Chat { channel_name: String, message: String },
  Leave { channel_name: String },
  Join { channel_name: String },
  Quit { }
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct Provider {
  pub name: String,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub emotes: HashMap<String, Emote>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub my_sub_emotes: HashSet<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub global_badges: Option<HashMap<String, Emote>>
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[derive(Default)]
#[derive(Eq, Hash, PartialEq)]
#[derive(Clone)]
pub enum ProviderName {
  #[default] Twitch,
  DGG,
  //YouTube,
}

pub struct ChatManager {
  handles: Vec<tokio::task::JoinHandle<()>>,
  pub in_tx: Sender<OutgoingMessage>,
  pub out_rx: Receiver<IncomingMessage>,
}

pub struct ChannelTransient {
  pub channel_emotes: Option<HashMap<String, Emote>>,
  pub badge_emotes: Option<HashMap<String, Emote>>,
  pub status: Option<ChannelStatus>
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct Channel {
  pub channel_name: String,
  pub roomid: String,
  pub provider: ProviderName,
  pub show_in_all: bool,
  pub send_history: VecDeque<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub send_history_ix: Option<usize>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub transient: Option<ChannelTransient>,
  pub users: HashSet<String>
}

#[derive(Clone)]
pub struct ComboCounter {
  pub word: String,
  pub count: usize,
  pub is_new: bool,
  pub is_end: bool
}

#[derive(Clone)]
pub struct ChatMessage {
  pub provider: ProviderName,
  pub channel: String,
  pub username: String,
  pub timestamp: DateTime<Utc>,
  pub message: String,
  pub profile: UserProfile,
  pub combo_data: Option<ComboCounter>,
  pub is_removed: bool
}

impl Default for ChatMessage {
  fn default() -> Self {
    Self { 
      provider: Default::default(), 
      channel: Default::default(), 
      username: Default::default(), 
      timestamp: Utc::now(), 
      message: Default::default(), 
      profile: Default::default(),
      combo_data: None,
      is_removed: false
    }
  }
}

#[derive(Clone, Default)]
pub struct UserProfile {
  pub badges: Option<Vec<String>>,
  pub display_name: Option<String>,
  pub color: Option<(u8, u8, u8)>
}

pub fn convert_color_hex(hex_string: Option<&String>) -> Option<(u8, u8, u8)> {
  match hex_string {
    Some(hex_str) => { 
      if hex_str.is_empty() {
        return None;
      }
      match hex::decode(hex_str.trim_start_matches('#')) {
        Ok(val) => Some((val[0], val[1], val[2])),
        Err(_) => {
          info!("ERROR {}", hex_str);
          None
        }
      }
    },
    None => None
  }
}

pub fn make_request(url: &str, headers: Option<Vec<(&str, String)>>, easy : &mut Easy) -> Result<String, anyhow::Error> {
  let mut result = String::default();

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
      String::from_utf8(data.to_vec()).map(|x| (&mut result).push_str(&x)).log_expect("failed to build string from http response body");
      Ok(data.len())
    })?;
    transfer.perform()?;
    drop(transfer);

    Ok(result)
}

#[derive(Clone,Debug,Default)]
pub struct ChannelStatus {
  pub game_name: Option<String>,
  pub is_live: bool,
  pub title: Option<String>,
  pub viewer_count: Option<usize>,
  pub started_at: Option<String>
}