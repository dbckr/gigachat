/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use async_channel::{Sender, Receiver};
//use rand::{distributions::{Alphanumeric, DistString}};
use tracing::info;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use curl::easy::Easy;
use tracing_unwrap::{ResultExt};

use crate::emotes::{Emote};

use self::channel::ChannelStatus;

pub mod twitch;
pub mod youtube_server;
pub mod dgg;
pub mod channel;

#[derive(Clone)]
pub enum IncomingMessage {
  PrivMsg { message: ChatMessage },
  EmoteSets { provider: ProviderName, emote_sets: Vec<String> },
  MsgEmotes { provider: ProviderName, emote_ids: Vec<(String, String)> },
  RoomId { channel: String, room_id: String },
  StreamingStatus { channel: String, status: Option<ChannelStatus>},
  UserJoin { channel: String, username: String, display_name: String },
  UserLeave { channel: String, username: String, display_name: String }
}

impl Default for IncomingMessage {
    fn default() -> Self {
        Self::PrivMsg { message: Default::default() }
    }
}

#[derive(Debug)]
pub enum OutgoingMessage {
  Chat { channel: String, message: String },
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
  pub global_badges: Option<HashMap<String, Emote>>,
  pub username: String
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[derive(Default)]
#[derive(Eq, Hash, PartialEq)]
#[derive(Clone)]
pub enum ProviderName {
  #[default] Twitch,
  DGG,
  YouTube,
}

pub trait ChatManagerRx {
  fn in_tx(&mut self) -> &mut Sender<OutgoingMessage>;
  fn out_rx(&mut self) -> &mut Receiver<IncomingMessage>;
}

pub struct ChatManager {
  handles: Vec<tokio::task::JoinHandle<()>>,
  pub username: String,
  pub in_tx: Sender<OutgoingMessage>,
  pub out_rx: Receiver<IncomingMessage>,
}
impl ChatManagerRx for ChatManager {
  fn in_tx(&mut self) -> &mut Sender<OutgoingMessage> {
    &mut self.in_tx
  }
  fn out_rx(&mut self) -> &mut Receiver<IncomingMessage> {
    &mut self.out_rx
  }
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
  pub is_removed: Option<String>,
  pub msg_type: MessageType
  //pub unique_id: String
}

#[derive(Default)]
#[derive(Clone)]
#[derive(Eq, Hash, PartialEq)]
pub enum MessageType {
  #[default] Chat,
  Error,
  Information,
  Announcement
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
      is_removed: None,
      msg_type: MessageType::Chat
      //unique_id: Alphanumeric.sample_string(&mut rand::thread_rng(), 16)
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
      String::from_utf8(data.to_vec()).map(|x| result.push_str(&x)).expect_or_log("failed to build string from http response body");
      Ok(data.len())
    })?;
    transfer.perform()?;
    drop(transfer);

    Ok(result)
}

pub fn display_system_message_in_chat(tx: &Sender<IncomingMessage>, channel: String, provider: ProviderName, message: String, msg_type: MessageType) {
  match tx.try_send(IncomingMessage::PrivMsg { message: ChatMessage {
    channel, 
    provider, 
    message,
    msg_type,
    ..Default::default() 
  } }) {
    Ok(_) => (),
    Err(x) => info!("Send failure for ERR: {}", x)
  };
}