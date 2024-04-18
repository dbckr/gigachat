/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::{HashMap, VecDeque};

use async_channel::Sender;

use crate::emotes::Emote;

use super::{ProviderName, ChatManager, ChatManagerRx, OutgoingMessage};

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct ChannelShared {
  pub channel_name: String,
  pub show_in_mentions_tab: bool,
  pub show_tab_when_offline: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub send_history: VecDeque<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub send_history_ix: Option<usize>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub transient: Option<ChannelTransient>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub users: HashMap<String, ChannelUser>,
}

impl ChannelShared {
  fn transient_mut(&mut self) -> Option<&mut ChannelTransient> {
    self.transient.as_mut()
  }
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub enum Channel {
  Twitch { twitch: TwitchChannel, shared: ChannelShared },
  DGG { dgg: DggChannel, shared: ChannelShared },
  Youtube { youtube: YoutubeChannel, shared: ChannelShared }
}

impl Channel {
  pub fn channel_name(&self) -> &String {
    match self {
      Channel::DGG { dgg: _, shared } => &shared.channel_name,
      Channel::Twitch { twitch: _, shared } => &shared.channel_name,
      Channel::Youtube { youtube: _, shared } => &shared.channel_name
    }
  }

  pub fn transient(&self) -> Option<&ChannelTransient> {
    match self {
      Channel::DGG { dgg: _, shared } => shared.transient.as_ref(),
      Channel::Twitch { twitch: _, shared } => shared.transient.as_ref(),
      Channel::Youtube { youtube: _, shared } => shared.transient.as_ref()
    }
  }

  pub fn transient_mut(&mut self) -> Option<&mut ChannelTransient> {
    match self {
      Channel::DGG { dgg: _, ref mut shared } => shared.transient_mut(),
      Channel::Twitch { twitch: _, ref mut shared } => shared.transient_mut(),
      Channel::Youtube { youtube: _, ref mut shared } => shared.transient_mut()
    }
  }

  pub fn shared(&self) -> &ChannelShared {
    match self {
      Channel::DGG { dgg: _, shared } => shared,
      Channel::Twitch { twitch: _, shared } => shared,
      Channel::Youtube { youtube: _, shared } => shared
    }
  }

  pub fn shared_mut(&mut self) -> &mut ChannelShared {
    match self {
      Channel::DGG { dgg: _, ref mut shared } => shared,
      Channel::Twitch { twitch: _, ref mut shared } => shared,
      Channel::Youtube { youtube: _, ref mut shared } => shared
    }
  }

  pub fn provider(&self) -> ProviderName {
    match self {
      Channel::DGG { dgg: _, shared: _ } => ProviderName::DGG,
      Channel::Twitch { twitch: _, shared: _ } => ProviderName::Twitch,
      Channel::Youtube { youtube: _, shared: _ } => ProviderName::YouTube
    }
  }
  
  pub fn close(&mut self) {
    match self {
      Channel::DGG { ref mut dgg, shared: _ } => if let Some(chat_mgr) = dgg.dgg_chat_manager.as_mut() { chat_mgr.close(); },
      Channel::Twitch { twitch: _, shared: _ } => {},
      Channel::Youtube { youtube: _, shared: _ } => {}
    }
  }

  pub fn chat_mgr_mut(&mut self) -> Option<&mut Sender<OutgoingMessage>> {
    match self {
      Channel::Twitch { twitch: _, shared: _ } => None,
      Channel::DGG { ref mut dgg, shared: _ } => dgg.dgg_chat_manager.as_mut().map(|m| m.in_tx()),
      Channel::Youtube { youtube: _, shared: _ } => None
    }
  }
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct TwitchChannel {
  pub room_id: Option<String>
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct DggChannel {
  pub dgg_chat_url: String, 
  pub dgg_status_url: String, 
  pub dgg_cdn_url: String,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub dgg_chat_manager: Option<ChatManager>
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct YoutubeChannel {
}

pub struct ChannelTransient {
  pub channel_emotes: Option<HashMap<String, Emote>>,
  pub badge_emotes: Option<HashMap<String, Emote>>,
  pub status: Option<ChannelStatus>
}

pub struct ChannelUser {
  pub username: String,
  pub display_name: String,
  pub is_active: bool
}

#[derive(Clone,Debug,Default)]
pub struct ChannelStatus {
  pub game_name: Option<String>,
  pub is_live: bool,
  pub title: Option<String>,
  pub viewer_count: Option<usize>,
  pub started_at: Option<String>
}