/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use tracing::{info, error, warn};
use tracing_unwrap::{OptionExt, ResultExt};
use std::{collections::{HashMap, VecDeque, vec_deque::IterMut}, ops::{Add}, iter::Peekable};
use chrono::{DateTime, Utc};
use egui::{emath::{Align, Rect}, RichText, Key, Modifiers, epaint::{FontId}, Rounding, Stroke, Pos2, Response};
use egui::{Vec2, ColorImage, FontDefinitions, FontData, text::LayoutJob, FontFamily, Color32};
use image::DynamicImage;
use itertools::Itertools;
use crate::{provider::{twitch::{self, TwitchChatManager}, ChatMessage, IncomingMessage, OutgoingMessage, Channel, Provider, ProviderName, ComboCounter, dgg, ChatManager, ChannelUser, MessageType, youtube_server, ChannelTransient, ChatManagerRx, ChannelStatus}, emotes::{imaging::load_file_into_buffer}, mod_selected_label::SelectableLabel};
use crate::{emotes, emotes::{Emote, EmoteLoader, EmoteStatus, EmoteRequest, EmoteResponse, imaging::{load_image_into_texture_handle, load_to_texture_handles}}};
use self::{chat::EmoteFrame, chat_estimate::TextRange};

#[cfg(instrumentation)]
use tracing::{instrument, trace_span};

pub mod chat;
pub mod chat_estimate;

const BUTTON_TEXT_SIZE : f32 = 20.0;
const BODY_TEXT_SIZE : f32 = 20.0;
const SMALL_TEXT_SIZE : f32 = 16.0;
/// Max length before manually splitting up a string without whitespace
const WORD_LENGTH_MAX : usize = 30;
/// Emotes in chat messages will be scaled to this height
pub const EMOTE_HEIGHT : f32 = 28.0;
const BADGE_HEIGHT : f32 = 18.0;
/// Should be at least equal to ui.spacing().interact_size.y
const MIN_LINE_HEIGHT : f32 = 22.0;
const COMBO_LINE_HEIGHT : f32 = 38.0;

pub enum ChannelTabDragEvent {
  MoveRight { channel : String },
  MoveLeft { channel : String },
  //MoveToRightPane,
  //MoveToLeftPane
}

pub struct UiChatMessageRow {
  pub row_height: f32,
  pub msg_char_range: TextRange,
  pub is_visible: bool,
  pub is_ascii_art: bool
}

pub struct UiChatMessage<'a> {
  pub message : &'a ChatMessage,
  pub emotes : HashMap<String, EmoteFrame>,
  pub badges : Option<Vec<(String, EmoteFrame)>>,
  pub mentions : Option<Vec<String>>,
  pub row_data : Vec<UiChatMessageRow>,
  pub msg_height : f32,
  pub user_color: Option<(u8,u8,u8)>,
  pub show_channel_name: bool,
  pub show_timestamp: bool
}

pub struct AddChannelMenu {
  channel_name: String,
  //channel_id: String,
  provider: ProviderName,
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
  channel_removed: Option<String>,
  state: ChatPanelOptions,
  y_size: f32
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))]
pub struct ChatPanelOptions {
  #[cfg_attr(feature = "persistence", serde(skip))]
  selected_channel: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  draft_message: String,
  #[cfg_attr(feature = "persistence", serde(skip))]
  chat_frame: Option<Rect>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  chat_scroll: Option<Vec2>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_user: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_msg: Option<(Vec2, ChatMessage)>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_emote: Option<String>,
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))]
pub struct TemplateApp {
  #[cfg_attr(feature = "persistence", serde(skip))]
  runtime: Option<tokio::runtime::Runtime>,
  pub providers: HashMap<ProviderName, Provider>,
  channels: HashMap<String, Channel>,
  pub auth_tokens: AuthTokens,
  enable_combos: bool,
  pub show_timestamps: bool,
  enable_yt_integration: bool,
  channel_tab_list: Vec<String>,
  selected_channel: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  rhs_selected_channel: Option<String>,
  pub lhs_chat_state: ChatPanelOptions,
  pub rhs_chat_state: ChatPanelOptions,
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
  pub dgg_chat_manager: Option<ChatManager>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub show_timestamps_changed: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub dragged_channel_tab: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  rhs_tab_width: Option<f32>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub yt_chat_manager: Option<ChatManager>
}

impl TemplateApp {
  pub fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Runtime) -> Self {
    cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
    let mut r = TemplateApp {
      ..Default::default()
    };
    #[cfg(feature = "persistence")]
    if let Some(storage) = cc.storage {
        r = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
    }
    r.emote_loader = EmoteLoader::new("Gigachat", &runtime);
    r.emote_loader.transparent_img = Some(load_image_into_texture_handle(&cc.egui_ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 0]) )))));
    r.runtime = Some(runtime);
    info!("{} channels", r.channels.len());

    if r.twitch_chat_manager.is_none() && !r.auth_tokens.twitch_username.is_empty() && !r.auth_tokens.twitch_auth_token.is_empty() {
      r.twitch_chat_manager = Some(TwitchChatManager::new(&r.auth_tokens.twitch_username, &r.auth_tokens.twitch_auth_token, r.runtime.as_ref().unwrap_or_log()));

      match r.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { token: r.auth_tokens.twitch_auth_token.to_owned(), force_redownload: false }) {  
        Ok(_) => {},
        Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
      };
    }
    /*if r.dgg_chat_manager.is_none() && let Some((_, sco)) = r.channels.iter_mut().find(|f| f.1.provider == ProviderName::DGG) {
      r.dgg_chat_manager = Some(dgg::open_channel(&r.auth_tokens.dgg_username, &r.auth_tokens.dgg_auth_token, sco, r.runtime.as_ref().unwrap_or_log(), &r.emote_loader));
    }*/
    r
  }
}

impl eframe::App for TemplateApp {
  #[cfg(feature = "persistence")]
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, eframe::APP_KEY, self);
  }

  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.update_inner(ctx);
  }

  fn on_exit(&mut self, _ctx : Option<&eframe::glow::Context>) {
    self.emote_loader.close();
    if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
      chat_mgr.close();
    }
    if let Some(chat_mgr) = self.dgg_chat_manager.as_mut() {
      chat_mgr.close();
    }
  }

  fn auto_save_interval(&self) -> std::time::Duration {
      std::time::Duration::from_secs(30)
  }

  fn max_size_points(&self) -> eframe::egui::Vec2 {
    eframe::egui::Vec2::new(1024.0, 2048.0)
  }

  fn clear_color(&self, _visuals : &eframe::egui::Visuals) -> eframe::egui::Rgba {
    eframe::egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200).into()
  }

  fn persist_native_window(&self) -> bool {
    true
  }

  fn persist_egui_memory(&self) -> bool {
    true
  }

  fn warm_up_enabled(&self) -> bool {
    false
  }
}

impl TemplateApp {
  #[cfg_attr(instrumentation, instrument(skip_all))]
  fn update_inner(&mut self, ctx: &egui::Context) {
    if self.emote_loader.transparent_img.is_none() {
      self.emote_loader.transparent_img = Some(load_image_into_texture_handle(ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 255]) )))));
    }

    if self.yt_chat_manager.is_none() && self.enable_yt_integration {
      self.yt_chat_manager = Some(youtube_server::start_listening(self.runtime.as_ref().unwrap()));

      for (name, channel) in self.channels.iter_mut().filter(|f| f.1.provider == ProviderName::YouTube) {
        channel.transient = Some(ChannelTransient {
          channel_emotes: None,
          badge_emotes: None,
          status: Some(ChannelStatus {
            title: Some(name.to_owned()),
            ..Default::default()
          })
        });
      }
    }

    // workaround for odd rounding issues at certain DPI(s?)
    if ctx.pixels_per_point() == 1.75 {
      ctx.set_pixels_per_point(1.50);
    }

    let set_emote_texture_data = |emote: &mut Emote, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>, loading_emotes: &mut HashMap<String, DateTime<Utc>>| {
      emote.data = load_to_texture_handles(ctx, data);
      emote.duration_msec = match emote.data.as_ref() {
        Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
        _ => 0,
      };
      emote.loaded = EmoteStatus::Loaded;
      emote.texture_expiration = None;//Some(chrono::Utc::now().add(chrono::Duration::hours(12)));
      loading_emotes.remove(&emote.name);
    };

    if let Ok(event) = self.emote_loader.rx.try_recv() {
      let loading_emotes = &mut self.emote_loader.loading_emotes;
      match event {
        EmoteResponse::GlobalEmoteListResponse { response } => {
          match response {
            Ok(x) => {
              for (name, emote) in x {
                self.global_emotes.insert(name, emote);
              }
            },
            Err(x) => { error!("ERROR LOADING GLOBAL EMOTES: {}", x); }
          };
        },
        EmoteResponse::GlobalEmoteImageLoaded { name, data } => {
          if let Some(emote) = self.global_emotes.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::TwitchGlobalBadgeListResponse { response } => {
          match response {
            Ok(badges) => {
              if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) {
                provider.global_badges = Some(badges)
              }
            },
            Err(e) => { error!("Failed to load twitch global badge json due to error {:?}", e); }
          }
        },
        EmoteResponse::GlobalBadgeImageLoaded { name, data } => {
          if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) 
          && let Some(global_badges) = &mut provider.global_badges && let Some(emote) = global_badges.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(emote) = channel.transient.as_mut()
          .and_then(|t| t.channel_emotes.as_mut()).and_then(|f| { f.get_mut(&name)}) {
            set_emote_texture_data(emote, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::ChannelBadgeImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(emote) = channel.transient.as_mut()
          .and_then(|t| t.badge_emotes.as_mut()).and_then(|f| { f.get_mut(&name)}) {
            set_emote_texture_data(emote, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::TwitchMsgEmoteLoaded { name, id: _, data } => {
          if let Some(p) = self.providers.get_mut(&ProviderName::Twitch) && let Some(emote) = p.emotes.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::YouTubeMsgEmoteLoaded { name, data } => {
          if let Some(p) = self.providers.get_mut(&ProviderName::YouTube) && let Some(emote) = p.emotes.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::TwitchEmoteSetResponse { emote_set_id: _, response } => {
          if let Ok(set_list) = response && let Some(provider) = self.providers.get_mut(&ProviderName::Twitch)  {
            for (_id, emote) in set_list {
              provider.my_sub_emotes.insert(emote.name.to_owned());
              if !provider.emotes.contains_key(&emote.name) {
                provider.emotes.insert(emote.name.to_owned(), emote);
              }
            }
          }
        },
        EmoteResponse::ChannelEmoteListResponse { channel_name, response } => {
          match response {
            Ok(emotes) => {
              if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(t) = channel.transient.as_mut() {
                t.channel_emotes = Some(emotes)
              }
            },
            Err(e) => { error!("Failed to load emote json for channel {} due to error {:?}", &channel_name, e); }
          }
        },
        EmoteResponse::ChannelBadgeListResponse { channel_name, response } => {
          match response {
            Ok(badges) => {
              if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(t) = channel.transient.as_mut() {
                t.badge_emotes = Some(badges)
              }
            },
            Err(e) => { error!("Failed to load badge json for channel {} due to error {:?}", &channel_name, e); }
          }
        }
      }
    }

    let mut styles = egui::Style::default();
    styles.text_styles.insert(
      egui::TextStyle::Small,
      FontId::new(/*18.0*/ SMALL_TEXT_SIZE, egui::FontFamily::Proportional));
    styles.text_styles.insert(
      egui::TextStyle::Body,
      FontId::new(/*18.0*/ BODY_TEXT_SIZE, egui::FontFamily::Proportional));
    styles.text_styles.insert(
      egui::TextStyle::Button,
      FontId::new(/*24.0*/ BUTTON_TEXT_SIZE, egui::FontFamily::Proportional));
    ctx.set_style(styles);

    self.ui_add_channel_menu(ctx);

    self.ui_auth_menu(ctx);
    
    let mut channel_removed = self.ui_channel_options(ctx);

    let mut msgs = 0;
    while let Some(chat_mgr) = self.twitch_chat_manager.as_mut() && let Ok(x) = chat_mgr.out_rx.try_recv() {
      self.handle_incoming_message(x);
      msgs += 1;
      if msgs > 20 { break; } // Limit to prevent bad UI lag
    }
    msgs = 0;
    while let Some(chat_mgr) = self.dgg_chat_manager.as_mut() && let Ok(x) = chat_mgr.out_rx.try_recv() {
      self.handle_incoming_message(x);
      msgs += 1;
      if msgs > 20 { break; } // Limit to prevent bad UI lag
    }
    msgs = 0;
    while let Some(chat_mgr) = self.yt_chat_manager.as_mut()  && let Ok(x) = chat_mgr.out_rx.try_recv() {
      self.handle_incoming_message(x);
      msgs += 1;
      if msgs > 20 { break; } // Limit to prevent bad UI lag
    }

    let mut channel_swap = false;
    let mut drag_channel_release : Option<String> = None;

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      ui.horizontal(|ui| {
        egui::menu::bar(ui, |ui| {
          if ui.menu_button(RichText::new("Add a channel").size(SMALL_TEXT_SIZE), |ui| { ui.close_menu(); }).response.clicked() {
            self.show_add_channel_menu = true;
          }
          ui.separator();
          if ui.menu_button(RichText::new("Configure Logins").size(SMALL_TEXT_SIZE), |ui| { ui.close_menu(); }).response.clicked() {
            self.show_auth_ui = true;
          }
          ui.separator();
          ui.menu_button(RichText::new("Options").size(SMALL_TEXT_SIZE), |ui| {
            ui.checkbox(&mut self.enable_combos, "Enable Combos");
            if ui.checkbox(&mut self.show_timestamps, "Show Message Timestamps").changed() {
              self.show_timestamps_changed = true;
            };
            ui.checkbox(&mut self.enable_yt_integration, "Enable YT Integration");
          });
          ui.separator();
          if ui.menu_button(RichText::new("View on Github").size(SMALL_TEXT_SIZE), |ui| { ui.close_menu(); }).response.clicked() {
            _ = ctx.output().open_url("https://github.com/dbckr/gigachat");
          }
          ui.separator();
          ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(SMALL_TEXT_SIZE).color(Color32::DARK_GRAY));
        });
      });
      ui.separator();

      ui.horizontal(|ui| {
        ui.horizontal_wrapped(|ui| {
          //let available_width = ui.available_width();
          if let Some(width) = self.rhs_tab_width {
            ui.set_max_width(ui.available_width() - width);
          }

          let label = RichText::new("Mentions").size(BUTTON_TEXT_SIZE);
          let clbl = ui.selectable_value(&mut self.selected_channel, None, label);
          if clbl.clicked() {
            channel_swap = true;
          }
          else if clbl.secondary_clicked() /*clbl.clicked_by(egui::PointerButton::Secondary)*/ {
            self.show_channel_options = Some((ctx.pointer_hover_pos().unwrap_or_log().to_vec2().to_owned(), "".to_owned()));
          }
    
          let mut tabs : Vec<(String, Response)> = Default::default();
          for channel in self.channel_tab_list.to_owned().iter() {  
            if self.rhs_selected_channel.as_ref() != Some(channel) {
              let clbl = self.ui_channel_tab(channel, ui, ctx, &mut channel_removed, &mut drag_channel_release);
              if let Some(clbl) = clbl {
                tabs.push((channel.to_owned(), clbl));
              }
            }
          }
    
          if self.dragged_channel_tab.is_some() && let Some(drag_channel) = self.dragged_channel_tab.as_ref() && let Some(ptr) = ctx.pointer_latest_pos() {
            let mut swapped = false;
            for ((l_channel, l_tab), (r_channel, r_tab)) in tabs.iter().tuple_windows() {
              if ui.min_rect().contains(ptr) {
                if l_channel == drag_channel && (ptr.x > r_tab.rect.left() && ptr.y > r_tab.rect.top() && ptr.y < r_tab.rect.bottom() || ptr.y > l_tab.rect.bottom()) {
                  let ix = self.channel_tab_list.iter().position(|x| x == l_channel);
                  if let Some(ix) = ix && ix < self.channel_tab_list.len() - 1 {
                    self.channel_tab_list.swap(ix, ix + 1);
                    swapped = true;
                  }
                }
                else if r_channel == drag_channel && (ptr.x < l_tab.rect.right() && ptr.y > l_tab.rect.top() && ptr.y < l_tab.rect.bottom() || ptr.y < r_tab.rect.top()) {
                  let ix = self.channel_tab_list.iter().position(|x| x == r_channel);
                  if let Some(ix) = ix && ix > 0 {
                    self.channel_tab_list.swap(ix - 1, ix);
                    swapped = true;
                  }
                }
              }
            }
            if !swapped && Some(drag_channel) != self.rhs_selected_channel.as_ref()
                && ctx.input().pointer.primary_clicked()
                && let Some(x) = tabs.iter_mut().find(|(name, _)| name == drag_channel)  {
              self.selected_channel = Some(drag_channel.to_owned());
              x.1.mark_changed();
              channel_swap = true;
            }
          }

          
        });
        if let Some(channel) = self.rhs_selected_channel.to_owned() {
            let resp = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
              self.ui_channel_tab(&channel, ui, ctx, &mut channel_removed, &mut drag_channel_release);
              if ui.button("<").on_hover_text("Close split chat").clicked() {
                self.rhs_selected_channel = None;
              }
            });
            self.rhs_tab_width = Some(resp.response.rect.width());
        }
      });
    });

    let lhs_chat_state = ChatPanelOptions {
        selected_channel: self.selected_channel.to_owned(),
        draft_message: self.lhs_chat_state.draft_message.to_owned(),
        chat_frame: self.lhs_chat_state.chat_frame.to_owned(),
        chat_scroll: self.lhs_chat_state.chat_scroll.to_owned(),
        selected_user: self.lhs_chat_state.selected_user.to_owned(),
        selected_msg: self.lhs_chat_state.selected_msg.to_owned(),
        selected_emote: self.lhs_chat_state.selected_emote.to_owned()
    };

    let mut popped_height = 0.;
    let mut rhs_popped_height = 0.;
    for (_channel, history) in self.chat_histories.iter_mut() {
      if history.len() > 2000 && let Some(popped) = history.pop_front() 
        && let Some(mut height) = popped.1 {
        if self.enable_combos && popped.0.combo_data.as_ref().is_some_and(|c| !c.is_end) {
          // add nothing to y_pos
        } else {
          if self.enable_combos && popped.0.combo_data.as_ref().is_some_and(|c| c.is_end && c.count > 1) {
            height = COMBO_LINE_HEIGHT + ctx.style().spacing.item_spacing.y;
          } 

          if self.selected_channel.is_none() || self.selected_channel.as_ref() == Some(&popped.0.channel) {
            popped_height += height;
          } else if self.rhs_selected_channel.is_none() || self.rhs_selected_channel.as_ref() == Some(&popped.0.channel) {
            rhs_popped_height += height;
          }
        }
      }
    }

    let cframe = egui::Frame { 
      inner_margin: egui::style::Margin::same(0.), 
      outer_margin: egui::style::Margin::same(3.),
      fill: egui::Color32::from_rgba_unmultiplied(40, 40, 40, 50),
      ..Default::default() 
    };
    let mut lhs_response : ChatFrameResponse = Default::default();
    /*let cpanel_resp =*/ egui::CentralPanel::default()
    .frame(cframe)
    .show(ctx, |ui| {
      let height = ui.available_height();
      ui.horizontal(|ui| {
        ui.set_height(height);
        lhs_response = self.show_chat_frame("lhs", ui, lhs_chat_state, ctx, self.rhs_selected_channel.is_some(), popped_height);
        if self.rhs_selected_channel.is_none() && let Some(pos) = ctx.pointer_latest_pos() && ui.min_rect().contains(pos) {
          if self.dragged_channel_tab.is_some() && pos.x > ui.available_width() * 0.5 {
            //paint rectangle to indicate drop will shift to other chat panel
            let paintrect = ui.max_rect().shrink2(Vec2::new(ui.max_rect().width() * 0.25, 0.)).translate(Vec2::new(ui.max_rect().width() * 0.25, 0.));
            ui.painter().rect_filled(paintrect, Rounding::none(), Color32::from_rgba_unmultiplied(40,40,40,150));
          }
          if let Some(channel) = drag_channel_release.as_ref() && pos.x > ui.available_width() * 0.5 && ui.min_rect().contains(pos) {
            self.rhs_selected_channel = Some(channel.to_owned());
            self.selected_channel = None;
          }
        }
        if self.rhs_selected_channel.is_some() {
          let rhs_chat_state = ChatPanelOptions {
            selected_channel: self.rhs_selected_channel.to_owned(),
            draft_message: self.rhs_chat_state.draft_message.to_owned(),
            chat_frame: self.rhs_chat_state.chat_frame.to_owned(),
            chat_scroll: self.rhs_chat_state.chat_scroll.to_owned(),
            selected_user: self.rhs_chat_state.selected_user.to_owned(),
            selected_msg: self.rhs_chat_state.selected_msg.to_owned(),
            selected_emote: self.rhs_chat_state.selected_emote.to_owned()
          };
          let rhs_response = self.show_chat_frame("rhs", ui, rhs_chat_state, ctx, false, rhs_popped_height);
          self.rhs_chat_state = rhs_response.state;

         //if drag_channel_release.is_some() {
         //  self.rhs_chat_state.chat_scroll = Some(Vec2 { x: 0., y:  rhs_response.y_size });
         //}
        }
      });
    });
    self.lhs_chat_state = lhs_response.state;

    if channel_swap {
      self.lhs_chat_state.chat_scroll = Some(Vec2 { x: 0., y:  lhs_response.y_size });
    }

    /*let rect = cpanel_resp.response.rect;
    if self.rhs_selected_channel.is_some() {
      let mut rhs_response : ChatFrameResponse = Default::default();
      let rhs_chat_state = ChatPanelOptions {
        selected_channel: self.rhs_selected_channel.to_owned(),
        draft_message: self.rhs_chat_state.draft_message.to_owned(),
        chat_frame: self.rhs_chat_state.chat_frame.to_owned(),
        chat_scroll: self.rhs_chat_state.chat_scroll.to_owned(),
        selected_user: self.rhs_chat_state.selected_user.to_owned(),
        selected_msg: self.rhs_chat_state.selected_msg.to_owned(),
        selected_emote: self.rhs_chat_state.selected_emote.to_owned()
      };

      egui::Window::new("RHS Chat")
      .frame(egui::Frame { 
        inner_margin: egui::style::Margin::same(0.), 
        outer_margin: egui::style::Margin { left: -3., right: 1., top: 0., bottom: 0. },
        fill: egui::Color32::TRANSPARENT,
        ..Default::default() 
      })
      .fixed_rect(Rect::from_two_pos(rect.center_top(), rect.right_bottom())
        .shrink2(Vec2::new(3., 0.))
        .translate(Vec2::new(5., 0.))
      )
      .title_bar(false)
      .collapsible(false)
      .show(ctx, |ui| {
        rhs_response = self.show_chat_frame("rhs", ui, rhs_chat_state, ctx, false, rhs_popped_height);
      });

      self.rhs_chat_state = rhs_response.state;
    }*/
    
    channel_removed = channel_removed.or(lhs_response.channel_removed);

    if let Some(channel) = channel_removed {
      if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
        chat_mgr.leave_channel(&channel);
      }
      self.channels.remove(&channel);
      self.channel_tab_list = self.channel_tab_list.iter().filter_map(|f| if f != &channel { Some(f.to_owned()) } else { None }).collect_vec();
    }

    ctx.request_repaint();
  }

  fn ui_channel_tab(&mut self, channel: &String, ui: &mut egui::Ui, ctx: &egui::Context, channel_removed: &mut Option<String>, drag_channel_release: &mut Option<String>) -> Option<Response> {
    if let Some(sco) = self.channels.get_mut(channel) {
      if let Some(t) = sco.transient.as_mut() {            
        let mut job = LayoutJob { ..Default::default() };
        job.append(if channel.len() > 16 { &channel[0..15] } else { channel }, 0., egui::TextFormat {
          font_id: FontId::new(BUTTON_TEXT_SIZE, FontFamily::Proportional), 
          color: Color32::LIGHT_GRAY,
          ..Default::default()
        });
        if channel.len() > 16 {
          job.append("..", 0., egui::TextFormat {
            font_id: FontId::new(BUTTON_TEXT_SIZE, FontFamily::Proportional), 
            color: Color32::LIGHT_GRAY,
            ..Default::default()
          });
        }
        if t.status.as_ref().is_some_and(|s| s.is_live) {
          let red = if self.selected_channel.as_ref() == Some(&sco.channel_name) { 255 } else { 200 };
          job.append("🔴", 3., egui::TextFormat {
            font_id: FontId::new(SMALL_TEXT_SIZE / 1.7, FontFamily::Proportional), 
            color: Color32::from_rgb(red, 0, 0),
            valign: Align::Center,
            ..Default::default()
          });
        }
        //let clbl = ui.selectable_value(&mut self.selected_channel, Some(channel.to_owned()), job);
        let clblx = SelectableLabel::new(self.selected_channel == Some(channel.to_owned()), job);
        let mut clbl = ui.add(clblx);
        
        if clbl.secondary_clicked() /*clbl.clicked_by(egui::PointerButton::Secondary)*/ {
          self.show_channel_options = Some((ctx.pointer_hover_pos().unwrap_or_log().to_vec2().to_owned(), channel.to_owned()));
        }
        else if clbl.middle_clicked() {
          *channel_removed = Some(channel.to_owned());
        }
        if clbl.drag_started() && self.dragged_channel_tab.is_none() {
          self.dragged_channel_tab = Some(channel.to_owned());
        }
        else if clbl.drag_released() {
          self.dragged_channel_tab = None;
          *drag_channel_release = Some(channel.to_owned());
        }

        let provider = match sco.provider {
          ProviderName::Twitch => "Twitch",
          ProviderName::DGG => "DGG Chat",
          ProviderName::YouTube => "YouTube"
        };

        //if t.status.is_some_and(|s| s.is_live) || channel.len() > 16 {
          clbl = clbl.on_hover_ui(|ui| {
            if let Some(status) = &t.status && status.is_live {
              ui.label(RichText::new(format!("{} ({})", channel, provider)).size(BODY_TEXT_SIZE * 1.5));
              if let Some(title) = status.title.as_ref() {
                ui.label(title);
              }
              if let Some(game) = status.game_name.as_ref() {
                ui.label(game);
              }
              if let Some(viewers) = status.viewer_count.as_ref() {
                ui.label(format!("{} viewers", viewers));
              }
          
              if let Some(started_at) = status.started_at.as_ref() { 
                if let Ok(dt) = DateTime::parse_from_rfc3339(started_at) {
                  let dur = chrono::Utc::now().signed_duration_since::<Utc>(dt.into()).num_seconds();
                  let width = 2;
                  ui.label(format!("Live for {:0width$}:{:0width$}:{:0width$}:{:0width$}", dur / 60 / 60 / 24, dur / 60 / 60 % 60, dur / 60 % 60, dur % 60));
                }
                else if let Ok(dt) = DateTime::parse_from_str(started_at, "%Y-%m-%dT%H:%M:%S%z") {
                  let dur = chrono::Utc::now().signed_duration_since::<Utc>(dt.into()).num_seconds();
                  let width = 2;
                  ui.label(format!("Live for {:0width$}:{:0width$}:{:0width$}:{:0width$}", dur / 60 / 60 / 24, dur / 60 / 60 % 60, dur / 60 % 60, dur % 60));
                }
              }
            }
            else {
              ui.label(format!("{} ({})", channel, provider));
            }
          });
        //}
        return Some(clbl);
      }
      else if sco.provider == ProviderName::Twitch && let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
        // channel has not been opened yet
        warn!("Failed to open channel: {}", channel);
        chat_mgr.open_channel(sco);
      }
      else if sco.provider == ProviderName::DGG {
        self.dgg_chat_manager = Some(dgg::open_channel(&self.auth_tokens.dgg_username, &self.auth_tokens.dgg_auth_token, sco, self.runtime.as_ref().unwrap_or_log(), &self.emote_loader));
      }
    }
    None
  }

  #[cfg_attr(instrumentation, instrument(skip_all))]
  fn show_chat_frame(&mut self, id: &str, ui: &mut egui::Ui, mut chat_panel: ChatPanelOptions, ctx: &egui::Context, half_width: bool, popped_height: f32) -> ChatFrameResponse {
    let mut response : ChatFrameResponse = Default::default();
    ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
      if half_width {
        ui.set_width(ui.available_width() / 2.);
      }
      //ui.painter().rect_stroke(ui.max_rect(), Rounding::none(), Stroke::new(2.0, Color32::DARK_RED));
      if let Some(sc) = chat_panel.selected_channel.as_ref().to_owned() {

        ui.style_mut().visuals.extreme_bg_color = Color32::from_rgba_premultiplied(0, 0, 0, 120);
        let mut outgoing_msg = egui::TextEdit::multiline(&mut chat_panel.draft_message)
          .desired_rows(2)
          .desired_width(ui.available_width())
          .hint_text("Type a message to send")
          .font(egui::TextStyle::Body)
          .show(ui);
          
        let goto_next_emote = chat_panel.selected_emote.is_some() && outgoing_msg.response.has_focus() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowRight);
        let goto_prev_emote = chat_panel.selected_emote.is_some() && outgoing_msg.response.has_focus() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowLeft);
        let enter_emote = chat_panel.selected_emote.is_some() && outgoing_msg.response.has_focus() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowDown);
        let prev_history = outgoing_msg.response.has_focus() && ui.input_mut().consume_key(Modifiers::NONE, Key::ArrowUp);
        let next_history = outgoing_msg.response.has_focus() && ui.input_mut().consume_key(Modifiers::NONE, Key::ArrowDown);

        if prev_history || next_history {
          if let Some(sco) = self.channels.get_mut(sc) {
            let mut ix = sco.send_history_ix.unwrap_or(0);
            let msg = sco.send_history.get(ix);
            if prev_history {
              ix = ix.add(1).min(sco.send_history.len() - 1);
            } else {
              ix = ix.saturating_sub(1);
            };
            if let Some(msg) = msg {
              chat_panel.draft_message = msg.to_owned();
              outgoing_msg.state.set_ccursor_range(
                Some(egui::text_edit::CCursorRange::one(egui::text::CCursor::new(chat_panel.draft_message.len())))
              );
            }
            sco.send_history_ix = Some(ix);
          }
        }

        if outgoing_msg.response.has_focus() && ui.input().key_down(egui::Key::Enter) && !ui.input().modifiers.shift && !chat_panel.draft_message.is_empty() {
          if let Some(sco) = self.channels.get_mut(sc) {
            let chat_tx = match sco.provider {
              ProviderName::Twitch => self.twitch_chat_manager.as_mut().map(|m| m.in_tx()),
              ProviderName::DGG => self.dgg_chat_manager.as_mut().map(|m| m.in_tx()),
              ProviderName::YouTube => self.yt_chat_manager.as_mut().map(|m| m.in_tx())
            };
            if let Some(chat_tx) = chat_tx {
              match chat_tx.try_send(OutgoingMessage::Chat { channel: sco.channel_name.to_owned(), message: chat_panel.draft_message.replace('\n', " ") }) {
                Err(e) => info!("Failed to send message: {}", e), //TODO: emit this into UI
                _ => {
                  sco.send_history.push_front(chat_panel.draft_message.to_owned());
                  chat_panel.draft_message = String::new();
                  sco.send_history_ix = None;
                }
              }
            }
          } 
        }
        else if !chat_panel.draft_message.is_empty() && let Some(cursor_pos) = outgoing_msg.state.ccursor_range() {
          let cursor = cursor_pos.primary.index;
          let msg = &chat_panel.draft_message.to_owned();
          let word : Option<(usize, &str)> = msg.split_whitespace()
            .map(move |s| (s.as_ptr() as usize - msg.as_ptr() as usize, s))
            .filter_map(|p| if p.0 <= cursor && cursor <= p.0 + p.1.len() { Some((p.0, p.1)) } else { None })
            .next();
          let is_user_list = word.as_ref().is_some_and(|f| f.1.starts_with('@'));
          let emotes = if is_user_list { self.get_possible_users(&chat_panel.selected_channel, word) } else { self.get_possible_emotes(&chat_panel.selected_channel, word) };
          if let Some((word, pos, emotes)) = emotes && !emotes.is_empty() {
            if enter_emote && let Some(emote_text) = &chat_panel.selected_emote {
              let msg = if chat_panel.draft_message.len() <= pos + word.len() || &chat_panel.draft_message[pos + word.len()..pos + word.len() + 1] != " " {
                format!("{}{} {}", &chat_panel.draft_message[..pos], emote_text, &chat_panel.draft_message[pos + word.len()..])
              } else {
                format!("{}{}{}", &chat_panel.draft_message[..pos], emote_text, &chat_panel.draft_message[pos + word.len()..])
              };
              chat_panel.draft_message = msg;
              outgoing_msg.response.request_focus();
              info!("{}", emote_text.len());
              outgoing_msg.state.set_ccursor_range(
                Some(egui::text_edit::CCursorRange::one(egui::text::CCursor::new(chat_panel.draft_message[..pos].len() + emote_text.len() + 1)))
              );
              chat_panel.selected_emote = None;
            }
            else {
              if goto_next_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == chat_panel.selected_emote.as_ref()) && ix + 1 < emotes.len() {
                chat_panel.selected_emote = emotes.get(ix + 1).map(|x| x.0.to_owned());
              }
              else if goto_prev_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == chat_panel.selected_emote.as_ref()) && ix > 0 {
                chat_panel.selected_emote = emotes.get(ix - 1).map(|x| x.0.to_owned());
              }
              else if chat_panel.selected_emote.is_none() || !emotes.iter().any(|x| Some(&x.0) == chat_panel.selected_emote.as_ref()) {
                chat_panel.selected_emote = emotes.first().map(|x| x.0.to_owned());
              }

              // Overlay style emote selector
              let msg_rect = outgoing_msg.response.rect.to_owned();
              let ovl_height = ui.available_height() / 2.;
              let painter_rect = msg_rect.expand2(egui::vec2(0., ovl_height)).translate(egui::vec2(0., (msg_rect.height() + ovl_height + 8.) * -1.));
              let mut painter = ui.painter_at(painter_rect);
              let painter_rect = painter.clip_rect();
              painter.set_layer_id(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new(format!("emoteselector {}", id))));

              let mut y = painter_rect.bottom();
              let mut x = painter_rect.left();
              for emote in emotes {
                let mut job = LayoutJob {
                  wrap: egui::epaint::text::TextWrapping { 
                    break_anywhere: false,
                    ..Default::default()
                  },
                  first_row_min_height: ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT),
                  ..Default::default()
                };
                job.append(&emote.0.to_owned(), 0., egui::TextFormat { 
                  font_id: FontId::new(BODY_TEXT_SIZE, FontFamily::Proportional),
                  ..Default::default() });
                let galley = ui.fonts().layout_job(job);
                let text_width = galley.rows.iter().map(|r| r.rect.width()).next().unwrap_or(16.) + 16.;

                let width = if !is_user_list {
                  let texture = emote.1.as_ref()
                    .and_then(|f| f.texture.as_ref())
                    .or_else(|| self.emote_loader.transparent_img.as_ref())
                    .unwrap_or_log();

                  let width = texture.size_vec2().x * (EMOTE_HEIGHT / texture.size_vec2().y);
                  if x + width + text_width > painter_rect.right() {
                    y -= EMOTE_HEIGHT;
                    x = painter_rect.left();
                  }

                  painter.rect_filled(egui::Rect {
                    min: egui::pos2(x, y - EMOTE_HEIGHT - 1.),
                    max: egui::pos2(x + width + text_width, y),
                  }, Rounding::none(), Color32::from_rgba_unmultiplied(20, 20, 20, 240));

                  let uv = egui::Rect::from_two_pos(egui::pos2(0., 0.), egui::pos2(1., 1.));
                  let rect = egui::Rect { 
                    min: egui::pos2(x, y - EMOTE_HEIGHT ), 
                    max: egui::pos2(x + width, y) 
                  };

                  let mut mesh = egui::Mesh::with_texture(texture.id());
                  mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
                  painter.add(egui::Shape::mesh(mesh));
                  width
                } else {
                  if x + text_width > painter_rect.right() {
                    y -= EMOTE_HEIGHT;
                    x = painter_rect.left();
                  }
                  painter.rect_filled(egui::Rect {
                    min: egui::pos2(x, y - EMOTE_HEIGHT - 1.),
                    max: egui::pos2(x + text_width + 1., y),
                  }, Rounding::none(), Color32::from_rgba_unmultiplied(20, 20, 20, 240));
                  0.
                };

                let disp_text = emote.0.to_owned();
                painter.text(
                  egui::pos2(x + width, y), 
                  egui::Align2::LEFT_BOTTOM, 
                  disp_text, 
                  FontId::new(BODY_TEXT_SIZE, egui::FontFamily::Proportional), 
                  if chat_panel.selected_emote == Some(emote.0) { Color32::RED } else { Color32::WHITE }
                );

                x = x + width + text_width;
              }
            }
          }
        }

        outgoing_msg.state.store(ctx, outgoing_msg.response.id);
      }
  
      ui.style_mut().visuals.override_text_color = Some(egui::Color32::LIGHT_GRAY);
      let selected_user_before = chat_panel.selected_user.as_ref().map(|x| x.to_owned());

      let chat_area = egui::ScrollArea::vertical()
        .id_source(format!("chatscrollarea {}", id))
        .auto_shrink([false; 2])
        .stick_to_bottom(true)
        .always_show_scroll(true)
        .scroll_offset(chat_panel.chat_scroll.map(|f| egui::Vec2 {x: 0., y: f.y - popped_height }).unwrap_or(egui::Vec2 {x: 0., y: 0.}));
  
      let mut overlay_viewport : Rect = Rect::NOTHING;
      let mut y_size = 0.;
      let area = chat_area.show_viewport(ui, |ui, viewport| {  
        overlay_viewport = viewport;
        y_size = self.show_variable_height_rows(&mut chat_panel, ui, viewport);
      });
      // if stuck to bottom, y offset at this point should be equal to scrollarea max_height - viewport height
      chat_panel.chat_scroll = Some(area.state.offset);

      let jump_rect = if area.state.offset.y != y_size - area.inner_rect.height() && y_size > area.inner_rect.height() {
        let rect = Rect {
          min: Pos2 { x: area.inner_rect.max.x - 60., y: area.inner_rect.max.y - 60. },
          max: area.inner_rect.max,
        };
        let jumpwin = egui::Window::new(format!("JumpToBottom {}", id))
        .fixed_rect(rect)
        .title_bar(false)
        .frame(egui::Frame { 
          inner_margin: egui::style::Margin::same(0.), 
          outer_margin: egui::style::Margin::same(0.),
          rounding: Rounding::none(), 
          shadow: eframe::epaint::Shadow::default(),
          fill: Color32::TRANSPARENT,
          stroke: Stroke::none()
        })
        .show(ctx, |ui| {
          if ui.button(RichText::new(" 🡳 ").size(48.)).clicked() {
            chat_panel.chat_scroll = Some(Vec2 { x: 0., y: y_size });
          }
        });
        jumpwin.unwrap_or_log().response.rect
      } else { Rect::NOTHING };

      response.y_size = y_size;

      // Overlay for selected chatter's history
      //self.selected_user_chat_history_overlay(area.inner_rect, ui);
      // Window for selected chatter's history
      let history_rect = self.selected_user_chat_history_window(id, &mut chat_panel, ui.max_rect(), ctx);
      if ctx.input().pointer.any_click()
          && selected_user_before == chat_panel.selected_user
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && area.inner_rect.contains(pos) 
          && !history_rect.contains(pos)
          && !jump_rect.contains(pos) {
        chat_panel.selected_user = None;
      }
    });
    response.state = chat_panel;
    response
  }

  fn ui_channel_options(&mut self, ctx: &egui::Context) -> Option<String> {
    let mut channel_removed : Option<String> = None;
    if self.show_channel_options.is_some() {
      let (pointer_vec, channel) = self.show_channel_options.to_owned().unwrap_or_log();
      let add_menu = egui::Window::new(format!("Configure Channel: {}", channel))
      .anchor(egui::Align2::LEFT_TOP, pointer_vec)
      .collapsible(false)
      .show(ctx, |ui| {
        if !channel.is_empty() {
          if ui.button("Remove channel").clicked() {
            channel_removed = Some(channel.to_owned());
            self.show_channel_options = None;
          }
          if ui.button("Reload channel emotes").clicked() {
            if let Some(ch) = self.channels.get_mut(&channel) {
              match ch.provider {
                ProviderName::Twitch => {
                  match self.emote_loader.tx.try_send(EmoteRequest::TwitchBadgeEmoteListRequest { 
                    channel_id: ch.roomid.to_owned(), 
                    channel_name: ch.channel_name.to_owned(),
                    token: self.auth_tokens.twitch_auth_token.to_owned(), 
                    force_redownload: true
                  }) {  
                    Ok(_) => {},
                    Err(e) => { error!("Failed to load emote json for channel {} due to error {:?}", &channel, e); }
                  };
                },
                ProviderName::DGG => {
                  match self.emote_loader.tx.try_send(EmoteRequest::DggFlairEmotesRequest { 
                    channel_name: ch.channel_name.to_owned(),
                    cdn_base_url: ch.dgg_cdn_url.to_owned(),
                    force_redownload: true
                  }) {
                    Ok(_) => {},
                    Err(e) => { error!("Failed to load badge/emote json for channel {} due to error {:?}", &channel, e); }
                  };
                },
                ProviderName::YouTube => {}
              };
            }
            self.show_channel_options = None;
          }
          if ui.button("Split screen").clicked() {
            self.rhs_selected_channel = Some(channel.to_owned());
            self.show_channel_options = None;
          }
        } else {
          let channels = self.channels.iter_mut();
          ui.label("Show mentions from:");
          for (name, channel) in channels {
            ui.checkbox(&mut channel.show_in_mentions_tab, name);
          }
        }
      }).unwrap_or_log();
      if ctx.input().pointer.any_click() 
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && !add_menu.response.rect.contains(pos) {
        self.show_channel_options = None;
      }
      else if ctx.input().key_pressed(Key::Escape) {
        self.show_channel_options = None;
      }
    }
    channel_removed
}

  fn ui_auth_menu(&mut self, ctx: &egui::Context) {
    let mut changed_twitch_token = false;
    let mut changed_dgg_token = false;
    if self.show_auth_ui {
      let auth_menu = egui::Window::new("Auth Tokens").collapsible(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
          ui.label("Twitch Username:");
          ui.text_edit_singleline(&mut self.auth_tokens.twitch_username);
        });
        ui.horizontal(|ui| {
          ui.label("Twitch Token:");
          if self.auth_tokens.show_twitch_auth_token {
            ui.text_edit_singleline(&mut self.auth_tokens.twitch_auth_token);
          }
          else if !self.auth_tokens.twitch_auth_token.is_empty() {
            ui.label("<Auth token hidden>");
          }
          else {
            ui.label("Not logged in");
          }
          if ui.button("Log In").clicked() {
            self.auth_tokens.twitch_auth_token = "".to_owned();
            self.auth_tokens.show_twitch_auth_token = true;
            twitch::authenticate(ctx, self.runtime.as_ref().unwrap_or_log());
          }
        });
        ui.separator();
        ui.horizontal(|ui| {
          ui.label("DGG Username:");
          ui.text_edit_singleline(&mut self.auth_tokens.dgg_username);
        });
        ui.horizontal(|ui| {
          ui.label("DGG Token:");
          if self.auth_tokens.show_dgg_auth_token {
            ui.text_edit_singleline(&mut self.auth_tokens.dgg_auth_token);
          }
          else if !self.auth_tokens.dgg_auth_token.is_empty() {
            ui.label("<Auth token hidden>");
          }
          else {
            ui.label("Not logged in");
          }
          if ui.button("Log In").clicked() {
            self.auth_tokens.dgg_auth_token = "".to_owned();
            self.auth_tokens.show_dgg_auth_token = true;
            ctx.output().open_url("https://www.destiny.gg/profile/developer");
            //self.auth_tokens.dgg_verifier = dgg::begin_authenticate(ctx);
          }
        });
        if self.auth_tokens.show_dgg_auth_token {
          ui.horizontal(|ui| {
            ui.label("  go to destiny.gg > Account > Developers > Connections > Add login key");
          });
        }
        /*ui.horizontal(|ui| {
          ui.label("YouTube");
          ui.text_edit_singleline(&mut self.auth_tokens.youtube_auth_token);
        });
        ui.separator();*/
        if ui.button("Ok").clicked() {
          changed_twitch_token = self.auth_tokens.show_twitch_auth_token;
          changed_dgg_token = self.auth_tokens.show_dgg_auth_token;
          let twitch_token = self.auth_tokens.twitch_auth_token.to_owned();
          if twitch_token.starts_with('#') || twitch_token.starts_with("access") {
            let rgx = regex::Regex::new("access_token=(.*?)&").unwrap_or_log();
            let cleaned = rgx.captures(twitch_token.as_str()).unwrap_or_log().get(1).map_or("", |x| x.as_str());
            self.auth_tokens.twitch_auth_token = cleaned.to_owned();
            if !cleaned.is_empty() {
              self.auth_tokens.show_twitch_auth_token = false;
            }
          }
          let dgg_token = self.auth_tokens.dgg_auth_token.to_owned();
          if dgg_token.starts_with('?') || dgg_token.starts_with("code") {
            let rgx = regex::Regex::new("code=(.*?)&").unwrap_or_log();
            let cleaned = rgx.captures(dgg_token.as_str()).unwrap_or_log().get(1).map_or("", |x| x.as_str());
            if !cleaned.is_empty() {
              let token = dgg::complete_authenticate(cleaned, &self.auth_tokens.dgg_verifier);
              self.auth_tokens.dgg_auth_token = token.expect_or_log("failed to get dgg token");
              self.auth_tokens.dgg_verifier = Default::default();
              self.auth_tokens.show_dgg_auth_token = false;
            }
          }
          else if !dgg_token.is_empty() {
            self.auth_tokens.show_dgg_auth_token = false;
          }
          self.show_auth_ui = false;

      
        }
      }).unwrap_or_log();
      if ctx.input().pointer.any_click() 
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && !auth_menu.response.rect.contains(pos) {
        self.show_auth_ui = false;
      }
      else if ctx.input().key_pressed(Key::Escape) {
        self.show_auth_ui = false;
      }
    }
    if changed_twitch_token {
      if let Some(mgr) = self.twitch_chat_manager.as_mut() {
        mgr.close();
      }
      if !self.auth_tokens.twitch_auth_token.is_empty() {
        let mut mgr = TwitchChatManager::new(&self.auth_tokens.twitch_username, &self.auth_tokens.twitch_auth_token, self.runtime.as_ref().unwrap_or_log());
        for (_, channel) in self.channels.iter_mut().filter(|(_, c)| c.provider == ProviderName::Twitch) {
          mgr.open_channel(channel);
        }
        self.twitch_chat_manager = Some(mgr);
        match self.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { token: self.auth_tokens.twitch_auth_token.to_owned(), force_redownload: false }) {  
          Ok(_) => {},
          Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
        };
      }
    }
    if changed_dgg_token {
      if let Some(mgr) = self.dgg_chat_manager.as_mut() {
        mgr.close();
      }

      if let Some((_, channel)) = self.channels.iter_mut().find(|(_, c)| c.provider == ProviderName::DGG) {
        let mgr = dgg::open_channel(&self.auth_tokens.dgg_username, &self.auth_tokens.dgg_auth_token, channel, self.runtime.as_ref().unwrap_or_log(), &self.emote_loader);
        self.dgg_chat_manager = Some(mgr);
      }            
    }
}

  fn ui_add_channel_menu(&mut self, ctx: &egui::Context) {
    let mut add_channel = |providers: &mut HashMap<ProviderName, Provider>, auth_tokens: &mut AuthTokens, channel_options: &mut AddChannelMenu| {
      let c = match channel_options.provider {
        ProviderName::Twitch => { 
          providers.entry(ProviderName::Twitch).or_insert(Provider {
              name: "twitch".to_owned(),
              my_sub_emotes: Default::default(),
              emotes: Default::default(),
              global_badges: Default::default(),
              username: Default::default()
          });
          match self.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { token: auth_tokens.twitch_auth_token.to_owned(), force_redownload: false }) {  
            Ok(_) => {},
            Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
          };
          if self.twitch_chat_manager.is_none() {
            self.twitch_chat_manager = Some(TwitchChatManager::new(&auth_tokens.twitch_username, &auth_tokens.twitch_auth_token, self.runtime.as_ref().unwrap_or_log()));
          }
          self.twitch_chat_manager.as_mut().unwrap_or_log().init_channel(&channel_options.channel_name)
        },
        ProviderName::DGG => dgg::init_channel(),
        ProviderName::YouTube => {
          providers.entry(ProviderName::YouTube).or_insert(Provider {
            name: "YouTube".to_owned(),
            my_sub_emotes: Default::default(),
            emotes: Default::default(),
            global_badges: Default::default(),
            username: Default::default()
          });

          Channel {
            ..Default::default()
          }
        }
        /*ProviderName::YouTube => {
          if providers.contains_key(&ProviderName::Twitch) == false {
            providers.insert(ProviderName::Twitch, Provider {
                name: "youtube".to_owned(),
                my_sub_emotes: Default::default(),
                emotes: Default::default(),
                global_badges: Default::default()
            });
          }
          youtube::init_channel(channel_options.channel_name.to_owned(), channel_options.channel_id.to_owned(), auth_tokens.youtube_auth_token.to_owned(), self.runtime.as_ref().unwrap_or_log())
        }*/
      };

      let name = c.channel_name.to_owned();
      self.channels.insert(name.to_owned(), c);
      self.channel_tab_list.push(name.to_owned());
      self.selected_channel = Some(name);
      channel_options.channel_name = Default::default();
    };
    if self.show_add_channel_menu {
      let add_menu = egui::Window::new("Add Channel").collapsible(false).show(ctx, |ui| {
        let mut name_input : Option<egui::Response> = None;
        ui.horizontal(|ui| {
          ui.label("Provider:");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::Twitch, "Twitch");
          //ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::YouTube, "Youtube");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::DGG, "destiny.gg");
        });
        if self.add_channel_menu.provider == ProviderName::Twitch {
          ui.horizontal(|ui| {
            ui.label("Channel Name:");
            name_input = Some(ui.text_edit_singleline(&mut self.add_channel_menu.channel_name));
            //name_input.request_focus();
          });
        }
        /*if self.add_channel_menu.provider == ProviderName::YouTube {
          ui.horizontal(|ui| {
            ui.label("Channel ID:");
            ui.text_edit_singleline(&mut self.add_channel_menu.channel_id);
          });
        }*/

        if name_input.is_some() && !self.add_channel_menu.channel_name.starts_with("YT:") && name_input.unwrap_or_log().has_focus() && ui.input().key_pressed(egui::Key::Enter) || ui.button("Add channel").clicked() {
          add_channel(&mut self.providers, &mut self.auth_tokens, &mut self.add_channel_menu); 
          self.show_add_channel_menu = false;
        }
        if ui.button("Cancel").clicked() {
          self.show_add_channel_menu = false;
        }
      }).unwrap_or_log();
      if ctx.input().pointer.any_click() 
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && !add_menu.response.rect.contains(pos) {
        self.show_add_channel_menu = false;
      }
      else if ctx.input().key_pressed(Key::Escape) {
        self.show_add_channel_menu = false;
      }
    }
}

  fn selected_user_chat_history_window(&mut self, id: &str, chat_panel: &mut ChatPanelOptions, area: Rect, ctx: &egui::Context) -> Rect {
    let ChatPanelOptions {
      selected_channel,
      draft_message: _,
      chat_frame: _,
      chat_scroll: _,
      selected_user,
      selected_msg,
      selected_emote: _
    } = chat_panel;

    let rect = area.to_owned()
        .shrink2(Vec2 { x: area.width() / 7., y: area.height() / 4.})
        .translate(egui::vec2(area.width() / 9., area.height() * -0.25));
    if selected_user.is_some() && let Some(channel) = selected_channel.as_ref() {
      let window = egui::Window::new(format!("Selected User History {}", id))
      .fixed_rect(rect)
      .title_bar(false)
      .show(ctx, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let chat_area = egui::ScrollArea::vertical()
          .auto_shrink([false, true])
          .stick_to_bottom(true);
        chat_area.show_viewport(ui, |ui, _viewport| {  
          let mut msgs = self.chat_histories.get(channel).unwrap_or_log().iter().rev()
          .filter_map(|(msg, _)| if selected_user.as_ref() == Some(&msg.username) || selected_user.as_ref() == msg.profile.display_name.as_ref() { Some(msg.to_owned()) } else { None })
          .take(4)
          .collect_vec();
          if msgs.is_empty() {
            ui.label(format!("No recent messages for user: {}", selected_user.as_ref().unwrap_or_log()));
          } else {
            msgs.reverse();
            let mut set_selected_msg : Option<ChatMessage> = None;
            for msg in &msgs {
              let transparent_texture = &mut self.emote_loader.transparent_img.as_ref().unwrap_or_log().to_owned();
              let est = create_uichatmessage(msg, ui, rect.width() - ui.spacing().item_spacing.x, false, self.show_timestamps, &self.providers, &self.channels, &self.global_emotes, &mut self.emote_loader);
              let (_, user_selected, msg_right_clicked) = chat::display_chat_message(ui, &est, &transparent_texture, None);
  
              if let Some(user) = user_selected.as_ref() {
                if selected_user.as_ref() == Some(user) {
                  *selected_user = None;
                } else {
                  *selected_user = Some(user.to_owned());
                }
              }
              if msg_right_clicked {
                set_selected_msg = Some(msg.to_owned());
              }
            }

            set_selected_message(set_selected_msg, ui, selected_msg);
          }
        });
      });

      window.unwrap_or_log().response.rect
    } else {
      Rect::NOTHING
    }
  }

#[cfg_attr(instrumentation, instrument(skip_all))]
  fn show_variable_height_rows(&mut self, chat_panel: &mut ChatPanelOptions, ui: &mut egui::Ui, viewport: Rect) -> f32 {
    let TemplateApp {
      runtime : _,
      providers,
      channels,
      auth_tokens : _,
      enable_combos,
      show_timestamps,
      channel_tab_list : _,
      selected_channel : _,
      rhs_selected_channel : _,
      lhs_chat_state : _,
      rhs_chat_state : _,
      chat_histories,
      show_add_channel_menu : _,
      add_channel_menu : _,
      global_emotes,
      emote_loader,
      show_auth_ui : _,
      show_channel_options : _,
      twitch_chat_manager : _,
      dgg_chat_manager : _,
      show_timestamps_changed,
      dragged_channel_tab : _,
      rhs_tab_width: _,
      yt_chat_manager: _,
      enable_yt_integration: _
    } = self;

    let ChatPanelOptions {
      selected_channel,
      draft_message: _,
      chat_frame,
      chat_scroll: _,
      selected_user,
      selected_msg,
      selected_emote: _
    } = chat_panel;

    let mut y_pos = 0.0;
    let mut set_selected_msg : Option<ChatMessage> = None;

    ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
      ui.spacing_mut().item_spacing.x = 4.0;
      //ui.spacing_mut().item_spacing.y = 1.;

      let y_min = ui.max_rect().top() + viewport.min.y;
      let y_max = ui.max_rect().top() + viewport.max.y;
      let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
      let mut in_view : Vec<UiChatMessage> = Default::default();
      let mut excess_top_space : Option<f32> = None;
      let mut skipped_rows = 0;

      let mut history_iters = Vec::new();
      for (cname, hist) in chat_histories.iter_mut() {
        if selected_channel.as_ref().is_some_and(|channel| channel == cname) || selected_channel.is_none() && channels.get(cname).is_some_and(|f| f.show_in_mentions_tab) {
          history_iters.push(hist.iter_mut().peekable());
        }
      }
      
      let mut history_iters = HistoryIterator {
        iterators: history_iters,
        //mentions_only: selected_channel.is_none(),
        //usernames: HashMap::default()// HashMap::from_iter(providers.iter().map(|(k, v)| (k.to_owned(), v.username.to_lowercase())))
      };
      let show_channel_names = history_iters.iterators.len() > 1;

      let mut usernames : HashMap<ProviderName, String> = HashMap::default();
      if selected_channel.is_none() {
        if let Some(twitch_chat_manager) = self.twitch_chat_manager.as_ref() {
          usernames.insert(ProviderName::Twitch, twitch_chat_manager.username.to_lowercase());
        }
        if let Some(dgg_chat_manager) = self.dgg_chat_manager.as_ref() {
          usernames.insert(ProviderName::DGG, dgg_chat_manager.username.to_lowercase());
        }
      }
      
      while let Some((row, cached_y)) = history_iters.get_next() {
        if selected_channel.is_none() && !mentioned_in_message(&usernames, &row.provider, &row.message) {
          continue;
        }

        let combo = &row.combo_data;

        // Skip processing if row size is accurately cached and not in view
        if !*show_timestamps_changed && let Some(last_viewport) = chat_frame && last_viewport.size() == viewport.size() && let Some(size_y) = cached_y.as_ref()
          && (y_pos < viewport.min.y - 1000. || y_pos + size_y > viewport.max.y + excess_top_space.unwrap_or(0.) + 1000.) {
            if *enable_combos && combo.as_ref().is_some_and(|c| !c.is_end) {
              // add nothing to y_pos
            } else if *enable_combos && combo.as_ref().is_some_and(|c| c.is_end && c.count > 1) {
              y_pos += COMBO_LINE_HEIGHT + ui.spacing().item_spacing.y;
            } else {
              y_pos += size_y;
            }
            if y_pos < viewport.min.y - 200. {
              skipped_rows += 1;
            }
            continue;
        }

        let mut uimsg = create_uichatmessage(row, ui, ui.available_width(), show_channel_names, *show_timestamps, providers, channels, global_emotes, emote_loader);
        let mut row_y = 0.;
        let mut has_visible = false;
        for line in uimsg.row_data.iter_mut() {
          let size_y = line.row_height;
          //info!("{} {}", viewport.min.y, viewport.max.y);
          if y_pos + row_y >= viewport.min.y && y_pos + row_y + size_y <= viewport.max.y + excess_top_space.unwrap_or(0.) {
            if excess_top_space.is_none() {
              excess_top_space = Some(y_pos + row_y - viewport.min.y);
            }
            line.is_visible = true;
            has_visible = true;
          }
          else {
            line.is_visible = false;
          }
          row_y += size_y + ui.spacing().item_spacing.y;
        }
        if *enable_combos && combo.as_ref().is_some_and(|c| !c.is_end) {
          // add nothing to y_pos
        } else if *enable_combos && combo.as_ref().is_some_and(|c| c.is_end && c.count > 1) {
          y_pos += COMBO_LINE_HEIGHT + ui.spacing().item_spacing.y;
        } else {
          y_pos += row_y;
        }
        *cached_y = Some(row_y);

        if has_visible {
          in_view.push(uimsg);
        }
      }

      if *show_timestamps_changed {
        *show_timestamps_changed = false;
      }

      let transparent_texture = emote_loader.transparent_img.as_ref().unwrap_or_log();
      *chat_frame = Some(viewport.to_owned());
      ui.set_height(y_pos);
      ui.skip_ahead_auto_ids(skipped_rows);
      //if *is_swap {
      //  ui.scroll_to_rect(Rect::from_min_size(Pos2 { x: 0., y: 0. }, Vec2 { x: 1., y: 1. }), None);
      //}

      ui.allocate_ui_at_rect(rect, |viewport_ui| {
        for chat_msg in in_view.iter() {
          if !*enable_combos || chat_msg.message.combo_data.is_none() || chat_msg.message.combo_data.as_ref().is_some_and(|c| c.is_end && c.count == 1) {
            let highlight_msg = match chat_msg.message.msg_type {
              MessageType::Announcement => Some(chat::get_provider_color(&chat_msg.message.provider).linear_multiply(0.25)),
              MessageType::Error => Some(Color32::from_rgba_unmultiplied(90, 0, 0, 90)),
              MessageType::Information => Some(Color32::TRANSPARENT),
              MessageType::Chat => if selected_user.as_ref() == Some(&chat_msg.message.profile.display_name.as_ref().unwrap_or(&chat_msg.message.username).to_lowercase()) {
                Some(Color32::from_rgba_unmultiplied(90, 90, 90, 90))
              } else {
                None
              }
            };
            let (_rect, user_selected, msg_right_clicked) = chat::display_chat_message(viewport_ui, chat_msg, transparent_texture, highlight_msg);

            if user_selected.is_some() {
              if *selected_user == user_selected {
                *selected_user = None
              } else {
                *selected_user = user_selected
              }
            }
            if msg_right_clicked {
              set_selected_msg = Some(chat_msg.message.to_owned());
            }
          }
          else if chat_msg.message.combo_data.as_ref().is_some_and(|combo| combo.is_end) { 
            chat::display_combo_message(viewport_ui, chat_msg, transparent_texture, show_channel_names, *show_timestamps);
          }
        }
      });
    });

    set_selected_message(set_selected_msg, ui, selected_msg);

    y_pos
  }

  fn handle_incoming_message(&mut self, x: IncomingMessage) {
    match x {
      IncomingMessage::PrivMsg { mut message } => {
        let provider_emotes = self.providers.get(&message.provider).map(|f| &f.emotes);
        let channel = message.channel.to_owned();
        // remove any extra whitespace between words
        let rgx = regex::Regex::new("\\s+").unwrap_or_log();
        message.message = rgx.replace_all(message.message.trim_matches(' '), " ").to_string();

        if message.provider == ProviderName::YouTube && !self.channels.contains_key(&message.channel) {
          self.channel_tab_list.push(message.channel.to_owned());
          self.channels.insert(message.channel.to_owned(), Channel { 
            channel_name: message.channel.to_owned(),  
            provider: ProviderName::YouTube, 
            transient: Some(ChannelTransient { 
              channel_emotes: None,
              badge_emotes: None,
              status: None }),
            ..Default::default() 
          });
        }

        if message.username.is_empty() && message.channel.is_empty() && message.msg_type != MessageType::Chat {
          let provider_channels = self.channels.iter().filter_map(|(_, c)| {
            if c.provider == message.provider { 
              Some(c.channel_name.to_owned())
            } else {
              None
            }
          }).collect_vec();
          for channel in provider_channels {
            let chat_history = self.chat_histories.entry(channel.to_owned()).or_insert_with(Default::default);
            push_history(
              chat_history, 
              message.to_owned(),
              provider_emotes, 
              self.channels.get(&channel).and_then(|f| f.transient.as_ref()).and_then(|f| f.channel_emotes.as_ref()),
              &self.global_emotes, 
              &mut self.emote_loader);
          }
        } else {
          let chat_history = self.chat_histories.entry(channel.to_owned()).or_insert_with(Default::default);

          if let Some(c) = self.channels.get_mut(&channel) {
            c.users.insert(message.username.to_lowercase(), ChannelUser {
              username: message.username.to_owned(),
              display_name: message.profile.display_name.as_ref().unwrap_or(&message.username).to_owned(),
              is_active: true
            });
            // Twitch has some usernames that have completely different display names (e.g. Japanese character display names)
            if let Some(display_name) = message.profile.display_name.as_ref() && display_name.to_lowercase() != message.username.to_lowercase() {
              c.users.insert(display_name.to_lowercase(), ChannelUser {
                username: message.username.to_owned(),
                display_name: message.profile.display_name.as_ref().unwrap_or(&message.username).to_owned(),
                is_active: true
              });
            }
          }

          push_history(
            chat_history, 
            message,
            provider_emotes, 
            self.channels.get(&channel).and_then(|f| f.transient.as_ref()).and_then(|f| f.channel_emotes.as_ref()),
            &self.global_emotes, 
            &mut self.emote_loader);
        }
      },
      IncomingMessage::StreamingStatus { channel, status } => {
        if let Some(t) = self.channels.get_mut(&channel).and_then(|f| f.transient.as_mut()) {
          t.status = status;
        }
      },
      IncomingMessage::MsgEmotes { provider, emote_ids } => {
        if let Some(p) = self.providers.get_mut(&provider) {
          for (id, name) in emote_ids {
            match provider {
              ProviderName::Twitch => if !p.emotes.contains_key(&name) {
                p.emotes.insert(name.to_owned(), Emote { name, id, url: "".to_owned(), path: "twitch/".to_owned(), ..Default::default() });
              },
              ProviderName::YouTube => if !p.emotes.contains_key(&name) {
                p.emotes.insert(id.to_owned(), Emote { id: id.to_owned(), name: id, url: name.to_owned(), path: "youtube/".to_owned(), ..Default::default() });
              },
              _ => ()
            }
          }
        }
      },
      IncomingMessage::RoomId { channel, room_id } => {
        if let Some(sco) = self.channels.get_mut(&channel) /*&& let Some(t) = sco.transient.as_mut()*/ {
          sco.roomid = room_id;
          match self.emote_loader.tx.try_send(EmoteRequest::TwitchBadgeEmoteListRequest { 
            channel_id: sco.roomid.to_owned(), 
            channel_name: sco.channel_name.to_owned(),
            token: self.auth_tokens.twitch_auth_token.to_owned(), 
            force_redownload: false
          }) {
            Ok(_) => {},
            Err(e) => warn!("Failed to request channel badge and emote list for {} due to error: {:?}", &channel, e)
          };

          //t.badge_emotes = emotes::twitch_get_channel_badges(&self.auth_tokens.twitch_auth_token, &sco.roomid, &self.emote_loader.base_path, true);
          //info!("loaded channel badges for {}:{}", channel, sco.roomid);
          //break;
        }
      },
      IncomingMessage::EmoteSets { provider,  emote_sets } => {
        //if let Some(provider) = self.providers.get_mut(&provider) {
        if provider == ProviderName::Twitch {
          for set in emote_sets {
            match self.emote_loader.tx.try_send(EmoteRequest::TwitchEmoteSetRequest { 
              token: self.auth_tokens.twitch_auth_token.to_owned(), 
              emote_set_id: set.to_owned(), 
              force_redownload: false 
            }) {
              Ok(_) => {},
              Err(e) => warn!("Failed to load twitch emote set {} due to error: {:?}", &set, e)
            };
          }
        }
      },
      IncomingMessage::UserJoin { channel, username, display_name } => {
        if let Some(c) = self.channels.get_mut(&channel) {
          // Usernames may have completely different display names (e.g. Japanese character display names)
          if display_name.to_lowercase() != username.to_lowercase() {
            c.users.insert(display_name.to_lowercase(), ChannelUser {
              username: username.to_owned(),
              display_name: display_name.to_owned(),
              is_active: true
            });
          }
          c.users.insert(username.to_lowercase(), ChannelUser {
            username,
            display_name,
            is_active: true
          });
        }
      },
      IncomingMessage::UserLeave { channel, username, display_name } => {
        if let Some(c) = self.channels.get_mut(&channel) {
          // Usernames may have completely different display names (e.g. Japanese character display names)
          if display_name.to_lowercase() != username.to_lowercase() {
            c.users.insert(display_name.to_lowercase(), ChannelUser {
              username: username.to_owned(),
              display_name: display_name.to_owned(),
              is_active: false
            });
          }
          c.users.insert(username.to_lowercase(), ChannelUser {
            username,
            display_name,
            is_active: false
          });
        }
      }
    };
  }

  #[cfg_attr(instrumentation, instrument(skip_all))]
  fn get_possible_emotes(&mut self, selected_channel: &Option<String>, word: Option<(usize, &str)>) -> Option<(String, usize, Vec<(String, Option<EmoteFrame>)>)> {
    if let Some((pos, input_str)) = word {
      if input_str.len() < 2  {
        return None;
      }
      let word = &input_str[0..];
      let word_lower = &word.to_lowercase();

      let mut starts_with_emotes : HashMap<String, Option<EmoteFrame>> = Default::default();
      let mut contains_emotes : HashMap<String, Option<EmoteFrame>> = Default::default();
      // Find similar emotes. Show emotes starting with same string first, then any that contain the string.
      if let Some(channel_name) = selected_channel && let Some(channel) = self.channels.get(channel_name) {
          if let Some(transient) = channel.transient.as_ref() && let Some(channel_emotes) = transient.channel_emotes.as_ref() {
          for (name, emote) in channel_emotes { // Channel emotes
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              let tex = chat::get_texture(&mut self.emote_loader, emote, EmoteRequest::new_channel_request(emote, channel_name));
              _ = match name_l.starts_with(word_lower) {
                true => starts_with_emotes.try_insert(name.to_owned(), Some(tex)),
                false => contains_emotes.try_insert(name.to_owned(), Some(tex)),
              };
            }
          }
        }
        if let Some(provider) = self.providers.get_mut(&channel.provider) { // Provider emotes
          for name in provider.my_sub_emotes.iter() {
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              if let Some(emote) = provider.emotes.get_mut(name) {
                let tex = chat::get_texture(&mut self.emote_loader, emote, EmoteRequest::new_twitch_emote_request(emote));
                _ = match name_l.starts_with(word_lower) {
                  true => starts_with_emotes.try_insert(name.to_owned(), Some(tex)),
                  false => contains_emotes.try_insert(name.to_owned(), Some(tex)),
                };
              }
            }
          }
        }
        // Global emotes, only if not DGG
        if channel.provider != ProviderName::DGG {
          for (name, emote) in &mut self.global_emotes { 
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              let tex = chat::get_texture(&mut self.emote_loader, emote, EmoteRequest::new_global_request(emote));
              _ = match name_l.starts_with(word_lower) {
                true => starts_with_emotes.try_insert(name.to_owned(), Some(tex)),
                false => contains_emotes.try_insert(name.to_owned(), Some(tex)),
              };
            }
          }
        }
      }
      
      let mut starts_with = starts_with_emotes.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).collect_vec();
      let mut contains = contains_emotes.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).collect_vec();
      starts_with.append(&mut contains);
      Some((input_str.to_owned(), pos, starts_with))
    }
    else {
      None
    }
  }

  #[cfg_attr(instrumentation, instrument(skip_all))]
  fn get_possible_users(&mut self, selected_channel: &Option<String>, word: Option<(usize, &str)>) -> Option<(String, usize, Vec<(String, Option<EmoteFrame>)>)> {
    if let Some((pos, input_str)) = word {
      if input_str.len() < 3  {
        return None;
      }
      let word = &input_str[1..];
      let word_lower = &word.to_lowercase();

      let mut starts_with_users : HashMap<String, Option<EmoteFrame>> = Default::default();
      let mut contains_users : HashMap<String, Option<EmoteFrame>> = Default::default();
      
      if let Some(channel_name) = selected_channel && let Some(channel) = self.channels.get_mut(channel_name) {
        for (name_lower, user) in channel.users.iter().filter(|(_k, v)| v.is_active) {
          if name_lower.starts_with(word_lower) || name_lower.contains(word_lower) {
            _ = match name_lower.starts_with(word_lower) {
              true => starts_with_users.try_insert(user.display_name.to_owned(), None),
              false => contains_users.try_insert(user.display_name.to_owned(), None),
            };
          }
        }
      }
      
      let mut starts_with = starts_with_users.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).collect_vec();
      let mut contains = contains_users.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).collect_vec();
      starts_with.append(&mut contains);
      Some((input_str.to_owned(), pos, starts_with))
    }
    else {
      None
    }
  }
}

#[cfg_attr(instrumentation, instrument(skip_all))]
fn create_uichatmessage<'a>(
  row: &'a ChatMessage,
  ui: &mut egui::Ui, 
  ui_width: f32,
  show_channel_name: bool,
  show_timestamp: bool,
  providers: &HashMap<ProviderName, Provider>,
  channels: &HashMap<String, Channel>,
  global_emotes: &HashMap<String, Emote>,
  emote_loader: &mut EmoteLoader) -> UiChatMessage<'a> {

  let (provider_emotes, provider_badges) = providers.get(&row.provider)
    .map(|p| (Some(&p.emotes), p.global_badges.as_ref())).unwrap_or((None, None));
  let (channel_emotes, channel_badges) = channels.get(&row.channel)
    .and_then(|c| c.transient.as_ref())
    .map(|t| (t.channel_emotes.as_ref(), t.badge_emotes.as_ref())).unwrap_or((None, None));

  let emotes = get_emotes_for_message(row, provider_emotes, channel_emotes, global_emotes, emote_loader);
  let (badges, user_color) = get_badges_for_message(row.profile.badges.as_ref(), &row.channel, provider_badges, channel_badges, emote_loader);
  let msg_sizing = chat_estimate::get_chat_msg_size(ui, ui_width, row, &emotes, badges.as_ref(), show_channel_name, show_timestamp);
  let mentions = if let Some(channel) = channels.get(&row.channel) {
    get_mentions_in_message(row, &channel.users)
  } else { None };

  let color = row.profile.color.or(user_color).map(|f| f.to_owned());
  let mut row_data : Vec<UiChatMessageRow> = Default::default();
  for (row_height, msg_char_range, is_ascii_art) in msg_sizing {
    row_data.push(UiChatMessageRow { row_height, msg_char_range, is_visible: true, is_ascii_art });
  }
  let msg_height = row_data.iter().map(|f| f.row_height).sum();

  UiChatMessage {
    message: row,
    emotes,
    badges,
    mentions,
    row_data,
    msg_height,
    user_color: color,
    show_channel_name,
    show_timestamp
  }
}

fn set_selected_message(set_selected_msg: Option<ChatMessage>, ui: &mut egui::Ui, selected_msg: &mut Option<(Vec2, ChatMessage)>) {
    let mut area = Rect::NOTHING;
    let mut clicked = false;
    if let Some(x) = set_selected_msg.as_ref() {
      let pos = ui.ctx().pointer_hover_pos().unwrap_or_log().to_vec2();
      *selected_msg = Some((Vec2 { x: pos.x, y: pos.y - ui.clip_rect().min.y}, x.to_owned()));
    }
    if let Some((pos, msg)) = selected_msg.as_ref() {
      (area, clicked) = msg_context_menu(ui, pos, msg);
    }
    if clicked || set_selected_msg.is_none() && ui.input().pointer.any_click() && ui.ctx().pointer_interact_pos().is_some() && !area.contains(ui.ctx().pointer_interact_pos().unwrap_or_log()) {
      *selected_msg = None;
    }
}

fn msg_context_menu(ui: &egui::Ui, point: &Vec2, msg: &ChatMessage) -> (Rect, bool) {
  let mut clicked = false;
  let window = egui::Window::new("ContextMenu")
  .anchor(egui::Align2::LEFT_TOP, point.to_owned())
  .title_bar(false)
  .show(ui.ctx(), |ui| {
    ui.spacing_mut().item_spacing.x = 4.0;
    let chat_area = egui::ScrollArea::vertical()
      .auto_shrink([true, true])
      .stick_to_bottom(true);
    chat_area.show_viewport(ui, |ui, _viewport| {  
      if ui.button("Copy Message").clicked() {
        ui.output().copied_text = msg.message.to_owned();
        clicked = true;
      }
    });
  });
  (window.unwrap_or_log().response.rect, clicked)
}

fn push_history(chat_history: &mut VecDeque<(ChatMessage, Option<f32>)>, mut message: ChatMessage, provider_emotes: Option<&HashMap<String, Emote>>, channel_emotes: Option<&HashMap<String, Emote>>, global_emotes: &HashMap<String, Emote>, emote_loader: &mut EmoteLoader) {
  let is_emote = !get_emotes_for_message(&message, provider_emotes, channel_emotes, global_emotes, emote_loader).is_empty();
  let last = chat_history.iter_mut().rev().find_or_first(|f| f.0.channel == message.channel);
  if let Some(last) = last && is_emote {
    let combo = combo_calculator(&message, last.0.combo_data.as_ref());
    if combo.as_ref().is_some_and(|c| !c.is_new && c.count > 1) && let Some(last_combo) = last.0.combo_data.as_mut() {
      last_combo.is_end = false; // update last item to reflect the continuing combo
    }
    else if last.0.combo_data.as_ref().is_some_and(|c| c.count <= 1) {
      last.0.combo_data = None;
    }
    message.combo_data = combo;
  } 
  else if is_emote {
    let combo = combo_calculator(&message, None);
    message.combo_data = combo;
  }
  chat_history.push_back((message, None));
}

fn combo_calculator(row: &ChatMessage, last_combo: Option<&ComboCounter>) -> Option<ComboCounter> { 
  if let Some(last_combo) = last_combo && last_combo.word == row.message.trim() {
    Some(ComboCounter {
        word: last_combo.word.to_owned(),
        count: last_combo.count + 1,
        is_new: false,
        is_end: true
    })
  }
  else if row.message.trim().contains(' ') {
    None
  }
  else {
    Some(ComboCounter {
      word: row.message.trim().to_owned(),
      count: 1,
      is_new: true,
      is_end: true
    })
  }
}

#[cfg_attr(instrumentation, instrument(skip_all))]
fn get_mentions_in_message(row: &ChatMessage, users: &HashMap<String, ChannelUser>) -> Option<Vec<String>> {
  Some(row.message.split(' ').into_iter().filter_map(|f| {
    let word = f.trim_start_matches('@').trim_end_matches(',').to_lowercase();
    users.get(&word).map(|u| u.display_name.to_owned())
  }).collect_vec())
}

#[cfg_attr(instrumentation, instrument(skip_all))]
fn get_emotes_for_message(row: &ChatMessage, provider_emotes: Option<&HashMap<String, Emote>>, channel_emotes: Option<&HashMap<String, Emote>>, global_emotes: &HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> HashMap<String, EmoteFrame> {
  let mut result : HashMap<String, chat::EmoteFrame> = Default::default();
  for word in row.message.to_owned().split(' ') {
    let emote = 
      if let Some(channel_emotes) = channel_emotes && let Some(emote) = channel_emotes.get(word) {
        Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_request(emote, &row.channel)))
      }
      else if row.provider != ProviderName::DGG && let Some(emote) = global_emotes.get(word) {
        Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_global_request(emote)))
      }
      else if let Some(provider_emotes) = provider_emotes && let Some(emote) = provider_emotes.get(word) {
        match row.provider {
          ProviderName::Twitch => Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_twitch_emote_request(emote))),
          ProviderName::YouTube => Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_youtube_emote_request(emote))),
          _ => None
        }
      }
      else {
        None
      };
    if let Some(frame) = emote {
      result.insert(word.to_owned(), frame);
    }
  }

  result
}

#[cfg_attr(instrumentation, instrument(skip_all))]
fn get_badges_for_message(badges: Option<&Vec<String>>, channel_name: &str, global_badges: Option<&HashMap<String, Emote>>, channel_badges: Option<&HashMap<String, Emote>>, emote_loader: &mut EmoteLoader) -> (Option<Vec<(String, EmoteFrame)>>, Option<(u8,u8,u8)>) {
  let mut result : Vec<(String, chat::EmoteFrame)> = Default::default();
  if badges.is_none() { return (None, None); }
  let mut greatest_badge : Option<(isize, (u8,u8, u8))> = None;
  for badge in badges.unwrap_or_log() {
    let emote = 
      if let Some(channel_badges) = channel_badges && let Some(emote) = channel_badges.get(badge) {
        if channel_name == dgg::DGG_CHANNEL_NAME {
          if emote.color.is_some() && (greatest_badge.is_none() || greatest_badge.is_some_and(|b| b.0 > emote.priority)) {
            greatest_badge = Some((emote.priority, emote.color.unwrap_or_log()))
          }
          if emote.hidden {
            continue;
          }
        }
        chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_badge_request(emote, channel_name))
      }
      else if let Some(global_badges) = global_badges && let Some(emote) = global_badges.get(badge) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_global_badge_request(emote))
      }
      else {
        EmoteFrame { id: badge.to_owned(), name: badge.to_owned(), label: None, path: badge.to_owned(), texture: None, zero_width: false }
      };
    
    result.push((emote.name.to_owned(), emote));
  }

  (Some(result), greatest_badge.map(|x| x.1))
}

pub fn load_font() -> FontDefinitions {
  let mut fonts = FontDefinitions::default();

  // Windows, use Segoe
  if let Some(font_file) = load_file_into_buffer("C:\\Windows\\Fonts\\segoeui.ttf") {
    let font = FontData::from_owned(font_file);
    fonts.font_data.insert("Segoe".into(), font);
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Segoe".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "Segoe".into());

    if let Some(emojis_font) = load_file_into_buffer("C:\\Windows\\Fonts\\seguiemj.ttf") {
      let emojis = FontData::from_owned(emojis_font);
      fonts.font_data.insert("emojis".into(), emojis);
      fonts.families.entry(FontFamily::Proportional).or_default().insert(1, "emojis".into());
      fonts.families.entry(FontFamily::Monospace).or_default().insert(1, "emojis".into());
    }

    // More windows specific fallback fonts for extended characters
    if let Some(symbols_font) = load_file_into_buffer("C:\\Windows\\Fonts\\seguisym.ttf") {
      let symbols = FontData::from_owned(symbols_font);
      fonts.font_data.insert("symbols".into(), symbols);
      fonts.families.entry(FontFamily::Proportional).or_default().push("symbols".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("symbols".into());
    }
    // Japanese
    if let Some(jp_font) = load_file_into_buffer("C:\\Windows\\Fonts\\simsunb.ttf.ttf") {
      let jp = FontData::from_owned(jp_font);
      fonts.font_data.insert("SimSun".into(), jp);
      fonts.families.entry(FontFamily::Proportional).or_default().push("SimSun".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("SimSun".into());
    }
    // Amogus
    if let Some(nirmala_font) = load_file_into_buffer("C:\\Windows\\Fonts\\Nirmala.ttf") {
      let nirmala = FontData::from_owned(nirmala_font);
      fonts.font_data.insert("Nirmala".into(), nirmala);
      fonts.families.entry(FontFamily::Proportional).or_default().push("Nirmala".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("Nirmala".into());
    }
  }
  // Non-windows, check for Roboto font
  else if let Some(font_file) = load_file_into_buffer("Roboto-Regular.ttf") {
    let mut font = FontData::from_owned(font_file);
    // tweak scale to make sizing similiar to Segoe
    font.tweak.scale = 0.88;
    fonts.font_data.insert("Roboto".into(), font);
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Roboto".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "Roboto".into());

    // Amogus font
    if let Some(nirmala_font) = load_file_into_buffer("NotoSansSinhala-Regular.ttf") {
      let nirmala = FontData::from_owned(nirmala_font);
      fonts.font_data.insert("NotoSansSinhala".into(), nirmala);
      fonts.families.entry(FontFamily::Proportional).or_default().push("NotoSansSinhala".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("NotoSansSinhala".into());
    }

    // Emoji font
    if let Some(emoji) = load_file_into_buffer("EmojiOneColor.otf") {
      let emojis = FontData::from_owned(emoji);
      fonts.font_data.insert("EmojiOneColor".into(), emojis);
      fonts.families.entry(FontFamily::Proportional).or_default().insert(1, "EmojiOneColor".into());
      fonts.families.entry(FontFamily::Monospace).or_default().insert(1, "EmojiOneColor".into());
    }
  }

  fonts
}

struct HistoryIterator<'a> {
  //histories: Vec<VecDeque<(ChatMessage, Option<f32>)>>,
  iterators: Vec<Peekable<IterMut<'a, (ChatMessage, Option<f32>)>>>,
  //mentions_only: bool,
  //usernames: HashMap<ProviderName, String>
}

impl<'a> HistoryIterator<'a> {
  fn get_next(&mut self) -> Option<&'a mut (ChatMessage, Option<f32>)> {
    let mut min_i = 0;
    let mut ts = Utc::now();
    //let usernames = &mut self.usernames;
    //let filtered_iters = self.iterators.iter_mut().map(|x| x.filter(|(msg, _)| !self.mentions_only || mentioned_in_message(usernames, &msg.provider, &msg.message)).peekable());
    let filtered_iters = self.iterators.iter_mut();
    let mut i = 0;
    for iter in filtered_iters {
      if let Some((msg, _y)) = iter.peek() && msg.timestamp < ts {
        ts = msg.timestamp.to_owned();
        min_i = i;
      }
      i += 1;
    }

    self.iterators.get_mut(min_i).and_then(|x| x.next())
  }
}

fn mentioned_in_message(usernames: &HashMap<ProviderName, String>, provider: &ProviderName, message : &String) -> bool {
  if let Some(username) = usernames.get(provider) {
    message.split(' ').into_iter().map(|f| {
      f.trim_start_matches('@').trim_end_matches(',').to_lowercase()
    }).any(|f| username == &f)
  } else {
    false
  }
}