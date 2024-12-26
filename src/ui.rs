use tracing::{info, error, warn, debug};
use tracing_unwrap::{OptionExt, ResultExt};
use std::{collections::{HashMap, VecDeque, vec_deque::IterMut}, ops::Add, iter::Peekable};
use chrono::{DateTime, Utc};
use egui::{emath::{Align, Rect}, epaint::FontId, text_edit::TextEditState, Context, Key, Modifiers, OpenUrl, Pos2, Response, RichText, Rounding, Stroke, TextStyle, TextureHandle};
use egui::{Vec2, FontDefinitions, FontData, text::LayoutJob, FontFamily, Color32};
use image::DynamicImage;
use itertools::Itertools;
use crate::{provider::{twitch::{self, TwitchChatManager}, ChatMessage, IncomingMessage, OutgoingMessage, Provider, ProviderName, ComboCounter, ChatManager, MessageType, youtube_server, ChatManagerRx, channel::{Channel, ChannelTransient, ChannelUser, YoutubeChannel, ChannelShared}, dgg}, emotes::{LoadEmote, AddEmote, OverlayItem, EmoteSource}};
use crate::emotes::imaging::load_file_into_buffer;
use crate::{emotes, emotes::{Emote, EmoteLoader, EmoteRequest, EmoteResponse, imaging::load_image_into_texture_handle}};
use self::models::TextRange;

mod template_app;

pub mod addtl_functions;
pub mod consts;
pub mod models;
pub mod chat;
pub mod chat_estimate;

mod channel_tabs;
mod chat_frame;
mod config_menus;
mod user_chat_history;
mod emote_selector;

use models::*;

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))]
pub struct TemplateApp {
  pub body_text_size : f32,
  bg_transparency: u8,
  chat_history_limit : usize,
  #[cfg_attr(feature = "persistence", serde(skip))]
  runtime: Option<tokio::runtime::Runtime>,
  pub providers: HashMap<ProviderName, Provider>,
  channels: HashMap<String, Channel>,
  pub auth_tokens: AuthTokens,
  enable_combos: bool,
  hide_offline_chats: bool,
  pub show_timestamps: bool,
  pub show_muted: bool,
  enable_yt_integration: bool,
  channel_tab_list: Vec<String>,
  selected_channel: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  rhs_selected_channel: Option<String>,
  pub lhs_chat_state: ChatPanelOptions,
  pub rhs_chat_state: ChatPanelOptions,
  force_compact_emote_selector: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  chat_histories: HashMap<String, VecDeque<(ChatMessage, Option<f32>)>>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  show_add_channel_menu: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  add_channel_menu: AddChannelMenu,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub global_emotes: HashMap<String, Emote>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub emote_loader: EmoteLoader,
  #[cfg_attr(feature = "persistence", serde(skip))]
  show_auth_ui: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  show_channel_options: Option<(Vec2, String)>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub twitch_chat_manager: Option<TwitchChatManager>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub show_timestamps_changed: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub dragged_channel_tab: DragChannelTabState,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub last_frame_ui_events: VecDeque<UiEvent>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  rhs_tab_width: Option<f32>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub yt_chat_manager: Option<ChatManager>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub discarded_last_frame: bool
}