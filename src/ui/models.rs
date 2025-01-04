/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use chrono::{DateTime, Utc};
use egui::{Color32, Pos2, Rect, Vec2};

use crate::{emotes::Emote, provider::{ChatMessage, ProviderName}};

use super::addtl_functions::get_provider_color;

use std::{collections::{vec_deque::IterMut, HashMap}, iter::Peekable, ops::{Range, RangeFrom}};

pub enum ChatPanel {
    Left,
    Right
}

#[derive(PartialEq)]
pub enum SelectorFormat {
    EmoteAndText,
    EmoteOnly,
    TextOnly
}

pub struct UiChatMessageRow {
  pub row_height: f32,
  pub msg_char_range: TextRange,
  pub is_visible: bool,
  pub is_ascii_art: bool
}

pub struct UiChatMessage<'a, 'b> {
  pub message : &'a ChatMessage,
  pub emotes : HashMap<String, &'b Emote>,
  pub badges : Option<Vec<&'b Emote>>,
  pub mentions : Option<Vec<String>>,
  pub row_data : Vec<UiChatMessageRow>,
  pub msg_height : f32,
  pub user_color: Option<(u8,u8,u8)>,
  pub show_channel_name: bool,
  pub show_timestamp: bool
}

impl<'a, 'b> UiChatMessage<'a, 'b> {
  pub fn channel_display_info(&'a self) -> Option<(&'a str, Color32)> {
    if self.show_channel_name {
      Some((&self.message.channel, get_provider_color(&self.message.provider)))
    } else {
      None
    }
  }

  pub fn username_display(&'a self) -> Option<(&'a String, Color32)> {
    self.message.get_username_with_color()
  }

  pub fn timestamp(&'a self) -> Option<&'a DateTime<Utc>> {
    if self.show_timestamp {
        Some(&self.message.timestamp)
    } else {
        None
    }
  }
}



pub struct AddChannelMenu {
  pub channel_name: String,
  //pub channel_id: String,
  pub provider: ProviderName,
}

impl Default for AddChannelMenu {
    fn default() -> Self {
        Self { 
          channel_name: Default::default(), 
          //channel_id: Default::default(), 
          provider: ProviderName::Twitch }
    }
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AuthTokens {
  pub twitch_username: String,
  pub twitch_auth_token: String,
  pub show_twitch_auth_token: bool,
  pub youtube_auth_token: String,
  pub show_dgg_auth_token: bool,
  pub dgg_username: String,
  pub dgg_auth_token: String,
  pub dgg_verifier: String
}

#[derive(Default)]
pub struct ChatFrameResponse {
  pub state: ChatPanelOptions,
  pub y_size: f32
}

#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))]
pub struct ChatPanelOptions {
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_channel: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub draft_message: String,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub chat_frame: Option<Rect>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub chat_scroll: Option<Vec2>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub chat_scroll_lock_to_bottom: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_user: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_msg: Option<(Vec2, ChatMessage)>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_emote: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_emote_input: Option<(usize, String)>,
}

impl Default for ChatPanelOptions {
    fn default() -> Self {
        ChatPanelOptions {
            chat_scroll_lock_to_bottom: true,
            
            selected_channel: None,
            draft_message: "".to_owned(),
            chat_frame: None,
            chat_scroll: None,
            selected_emote: None,
            selected_emote_input: None,
            selected_msg: None,
            selected_user: None
        }
    }
}

#[derive(Default)]
#[derive(PartialEq)]
pub enum DragChannelTabState {
    #[default]
    None,
    DragStart(String, Vec<String>),
    DragRelease(String, bool, Pos2)
}

pub enum UiEvent {
    ChannelChangeLHS,
    ChannelChangeRHS,
    EmoteSelectionEntered(usize),
    ChannelRemoved(String)
}

#[derive(Debug)]
pub enum TextRange {
  Range { range: Range<usize> },
  EndRange { range: RangeFrom<usize> }
}

impl TextRange {
  pub fn start(&self) -> usize {
    match self {
      TextRange::Range { range } => range.start,
      TextRange::EndRange { range } => range.start
    }
  }
}

pub struct HistoryIterator<'a> {
    //histories: Vec<VecDeque<(ChatMessage, Option<f32>)>>,
    pub iterators: Vec<Peekable<IterMut<'a, (ChatMessage, Option<f32>)>>>,
    //mentions_only: bool,
    //usernames: HashMap<ProviderName, String>
  }
  
  impl<'a> HistoryIterator<'a> {
    pub fn get_next(&mut self) -> Option<&'a mut (ChatMessage, Option<f32>)> {
      let mut min_i = 0;
      let mut ts = Utc::now();
      //let usernames = &mut self.usernames;
      //let filtered_iters = self.iterators.iter_mut().map(|x| x.filter(|(msg, _)| !self.mentions_only || mentioned_in_message(usernames, &msg.provider, &msg.message)).peekable());
      let filtered_iters = self.iterators.iter_mut();
      for (i, iter) in filtered_iters.enumerate() {
        if let Some((msg, _y)) = iter.peek() && msg.timestamp < ts {
          ts.clone_from(&msg.timestamp);
          min_i = i;
        }
      }
  
      self.iterators.get_mut(min_i).and_then(|x| x.next())
    }
  }