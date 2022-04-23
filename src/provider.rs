/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use eframe::epaint::Color32;
use tokio::{sync::mpsc, task::JoinHandle};

use crate::emotes::Emote;

pub mod twitch;

#[derive(Clone)]
pub enum InternalMessage {
  PrivMsg { message: ChatMessage },
  EmoteSets { emote_sets: Vec<String> },
  MsgEmotes { emote_ids: Vec<(String, String)> },
  RoomId { room_id: String },
}

pub enum OutgoingMessage {
  Chat { message: String },
  Leave {},
}

pub struct Provider {
  pub provider: String,
  pub emotes: HashMap<String, Emote>,
  pub emote_sets: HashMap<String,HashMap<String,Emote>>,
}

pub struct Channel {
  pub channel_name: String,
  pub roomid: String,
  pub provider: String,
  pub history: Vec<ChatMessage>,
  pub history_viewport_size_y: f32,
  pub rx: mpsc::Receiver<InternalMessage>,
  pub tx: mpsc::Sender<OutgoingMessage>,
  pub channel_emotes: HashMap<String, Emote>,
  pub task_handle: Option<JoinHandle<()>>
}

impl Channel {
  pub async fn close(&mut self) {
    let Self {
        channel_name : _,
        roomid : _,
        provider : _,
        history : _,
        history_viewport_size_y : _,
        tx,
        rx : _,
        channel_emotes : _,
        task_handle
    } = self;

    if let Some(handle) = task_handle {
      if tx.send(OutgoingMessage::Leave {  }).await.is_ok() {
        match handle.await {
          Ok(_) => (),
          Err(e) => println!("{:?}", e)
        }
      }
    }
  }
}

#[derive(Clone)]
pub struct ChatMessage {
  pub username: String,
  pub timestamp: DateTime<Utc>,
  pub message: String,
  pub profile: UserProfile 
}

impl Default for ChatMessage {
  fn default() -> Self {
    Self {
      username: Default::default(),
      timestamp: Utc::now(),
      message: Default::default(),
      profile: Default::default()
    }
  }
}

#[derive(Clone)]
pub struct UserProfile {
  pub badges: Vec<UserBadge>,
  pub display_name: Option<String>,
  pub color: (u8, u8, u8)
}

impl Default for UserProfile {
  fn default() -> Self {
    Self {
      color: (255, 255, 255),
      display_name: Default::default(),
      badges: Vec::new()
    }
  }
}

#[derive(Clone)]
pub struct UserBadge {
  pub image_data: Vec<u8>
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

pub fn convert_color(input : &(u8, u8, u8)) -> Color32 {
  // return white
  if input == &(255u8, 255u8, 255u8) {
    return Color32::WHITE;
  }

  // normalize brightness
  let target = 150;
 
  let min = |x, y| -> u8 {
    let z = x < y;
    match z {
      true => x,
      _ => y
    }
  };

  let tf = |x| -> (u8, u8) {
    if x < target {
      (target - x, 255 - x)
    }
    else {
      (0, 255 - x)
    }
  };

  let (r, g, b) = (input.0, input.1, input.2);

  let (r_diff, r_max_adj) = tf(r);
  let (g_diff, g_max_adj) = tf(g);
  let (b_diff, b_max_adj) = tf(b);

  let adj = ((r_diff as u16 + g_diff as u16 + b_diff as u16) / 3) as u8;

  let (rx, gx, bx) = (r + min(adj, r_max_adj), g + min(adj, g_max_adj), b + min(adj, b_max_adj));

  //println!("{} {} {}", rx, gx, bx);
  return Color32::from_rgb(rx, gx, bx);
}