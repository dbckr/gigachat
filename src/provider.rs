/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::{HashMap, VecDeque, HashSet};

use chrono::{DateTime, Utc};
use tokio::{sync::mpsc, task::JoinHandle, runtime::Runtime};

use crate::emotes::{Emote, EmoteLoader};

pub mod twitch;
//pub mod youtube;

#[derive(Clone)]
pub enum InternalMessage {
  PrivMsg { message: ChatMessage },
  EmoteSets { emote_sets: Vec<String> },
  MsgEmotes { emote_ids: Vec<(String, String)> },
  RoomId { room_id: String },
  StreamingStatus { is_live: bool}
}

impl Default for InternalMessage {
    fn default() -> Self {
        Self::PrivMsg { message: Default::default() }
    }
}

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
  //YouTube,
}

pub struct ChannelTransient {
  pub channel_emotes: Option<HashMap<String, Emote>>,
  pub badge_emotes: Option<HashMap<String, Emote>>,
  //pub task_handle: JoinHandle<()>,
  pub is_live: bool
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct Channel {
  pub channel_name: String,
  pub roomid: String,
  pub provider: ProviderName,
  pub send_history: VecDeque<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub send_history_ix: Option<usize>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub transient: Option<ChannelTransient>
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
  pub combo_data: Option<ComboCounter>
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
      combo_data: None
    }
  }
}

#[derive(Clone)]
pub struct UserProfile {
  pub badges: Option<Vec<String>>,
  pub display_name: Option<String>,
  pub color: (u8, u8, u8)
}

impl Default for UserProfile {
  fn default() -> Self {
    Self {
      color: (255, 255, 255),
      display_name: Default::default(),
      badges: None
    }
  }
}

pub fn convert_color_hex(hex_string: Option<&String>) -> (u8, u8, u8) {
  match hex_string {
    Some(hex_str) => { 
      if hex_str == "" {
        return (255,255,255)
      }
      match hex::decode(hex_str.trim_start_matches("#")) {
        Ok(val) => (val[0], val[1], val[2]),
        Err(_) => {
          println!("ERROR {}", hex_str);
          (255, 255, 255)
        }
      }
    },
    None => (255, 255, 255)
  }
}