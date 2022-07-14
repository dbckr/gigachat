/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::{HashMap, VecDeque, vec_deque::IterMut}, ops::{Add}, iter::Peekable};
use chrono::{DateTime, Utc};
use egui::{emath::{Align, Rect}, RichText, Key, Modifiers, epaint::{FontId}, Rounding};
use egui::{Vec2, ColorImage, FontDefinitions, FontData, text::LayoutJob, FontFamily, Color32};
use image::DynamicImage;
use itertools::Itertools;
use crate::{provider::{twitch::{self, TwitchChatManager}, ChatMessage, IncomingMessage, OutgoingMessage, Channel, Provider, ProviderName, ComboCounter, dgg, ChatManager}, emotes::imaging::load_file_into_buffer};
use crate::{emotes, emotes::{Emote, EmoteLoader, EmoteStatus, EmoteRequest, EmoteResponse, imaging::{load_image_into_texture_handle, load_to_texture_handles}}};
use self::{chat::EmoteFrame, chat_estimate::TextRange};

pub mod chat;
pub mod chat_estimate;

const BUTTON_TEXT_SIZE : f32 = 20.0;
const BODY_TEXT_SIZE : f32 = 18.0;
const SMALL_TEXT_SIZE : f32 = 15.0;
/// Max length before manually splitting up a string without whitespace
const WORD_LENGTH_MAX : usize = 30;
/// Emotes in chat messages will be scaled to this height
pub const EMOTE_HEIGHT : f32 = 28.0;
const BADGE_HEIGHT : f32 = 18.0;
/// Should be at least equal to ui.spacing().interact_size.y
const MIN_LINE_HEIGHT : f32 = 22.0;
const COMBO_LINE_HEIGHT : f32 = 38.0;

pub struct UiChatMessageRow {
  pub row_height: f32,
  pub msg_char_range: TextRange,
  pub is_visible: bool
}

pub struct UiChatMessage<'a> {
  pub message : &'a ChatMessage,
  pub emotes : HashMap<String, EmoteFrame>,
  pub badges : Option<HashMap<String, EmoteFrame>>,
  pub row_data : Vec<UiChatMessageRow>,
  pub msg_height : f32,
  pub is_ascii_art: bool
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
  pub dgg_auth_token: String,
  pub dgg_verifier: String
}

#[derive(Default)]
#[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(default))]
pub struct TemplateApp {
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  runtime: Option<tokio::runtime::Runtime>,
  pub providers: HashMap<ProviderName, Provider>,
  channels: HashMap<String, Channel>,
  selected_channel: Option<String>,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  chat_histories: HashMap<String, VecDeque<(ChatMessage, Option<f32>)>>,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  draft_message: String,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  show_add_channel_menu: bool,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  add_channel_menu: AddChannelMenu,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  pub global_emotes: HashMap<String, Emote>,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  pub emote_loader: Option<EmoteLoader>,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  pub selected_emote: Option<String>,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  show_auth_ui: bool,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  show_channel_options: bool,
  pub auth_tokens: AuthTokens,
  chat_frame: Option<Rect>,
  chat_scroll: Option<Vec2>,
  enable_combos: bool,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  pub twitch_chat_manager: Option<TwitchChatManager>,
  #[cfg_attr(all(feature = "persistence", not(feature = "use-bevy")), serde(skip))]
  pub dgg_chat_manager: Option<ChatManager>
}

#[cfg(feature = "use-bevy")]
pub fn bevy_update(mut egui_ctx: bevy::prelude::ResMut<bevy_egui::EguiContext>,
  mut ui_state: bevy::prelude::ResMut<TemplateApp>) {
    ui_state.update_inner(egui_ctx.ctx_mut())
}

#[cfg(feature = "use-bevy")]
pub fn bevy_configure_visuals(mut egui_ctx: bevy::prelude::ResMut<bevy_egui::EguiContext>) {
  egui_ctx.ctx_mut().set_visuals(egui::Visuals::dark());
  egui_ctx.ctx_mut().set_fonts(load_font());
  egui_ctx.ctx_mut().set_pixels_per_point(1.0);
}

#[cfg(feature = "use-bevy")]
pub fn bevy_update_ui_scale_factor(
  _keyboard_input: bevy::prelude::Res<bevy::input::Input<bevy::prelude::KeyCode>>,
  mut _toggle_scale_factor: bevy::prelude::Local<Option<bool>>,
  mut egui_settings: bevy::prelude::ResMut<bevy_egui::EguiSettings>,
  _windows: bevy::prelude::Res<bevy::window::Windows>,) 
{
    egui_settings.scale_factor = 0.80;
}

impl TemplateApp {
  #[cfg(not(feature = "use-bevy"))]
  pub fn new(cc: &eframe::CreationContext<'_>, title: String, runtime: tokio::runtime::Runtime) -> Self {
    cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
    let mut r = TemplateApp {
      ..Default::default()
    };
    #[cfg(feature = "persistence")]
    if let Some(storage) = cc.storage {
        r = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
    }
    let mut loader = EmoteLoader::new(&title, &runtime);
    loader.transparent_img = Some(load_image_into_texture_handle(&cc.egui_ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 255]) )))));
    r.runtime = Some(runtime);
    r.emote_loader = Some(loader);
    println!("{} channels", r.channels.len());
    r
  }

  #[cfg(feature = "use-bevy")]
  pub fn new(title: String, runtime: tokio::runtime::Runtime) -> Self {
    let mut r = TemplateApp {
      ..Default::default()
    };
    
    let loader = EmoteLoader::new(&title, &runtime);
    r.runtime = Some(runtime);
    r.emote_loader = Some(loader);
    println!("{} channels", r.channels.len());
    r
  }
}

#[cfg(not(feature = "use-bevy"))]
impl eframe::App for TemplateApp {
  #[cfg(feature = "persistence")]
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, eframe::APP_KEY, self);
  }

  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.update_inner(ctx)
  }

  fn on_exit_event(&mut self) -> bool {
    true
  }

  fn on_exit(&mut self, _ctx : &eframe::glow::Context) {
    self.emote_loader.as_ref().unwrap().close();
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
    eframe::egui::Color32::from_rgba_premultiplied(0, 0, 0, 200).into()
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
  fn update_inner(&mut self, ctx: &egui::Context) {

    if self.emote_loader.as_ref().unwrap().transparent_img == None {
      self.emote_loader.as_mut().unwrap().transparent_img = Some(load_image_into_texture_handle(ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 255]) )))));
    }

    #[cfg(not(feature="use-bevy"))]
    if ctx.pixels_per_point() == 1.75 {
      ctx.set_pixels_per_point(1.50);
    }

    let set_emote_texture_data = |emote: &mut Emote, ctx: &egui::Context, data: Option<Vec<(ColorImage, u16)>>| {
      emote.data = load_to_texture_handles(ctx, data);
      emote.duration_msec = match emote.data.as_ref() {
        Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
        _ => 0,
      };
      emote.loaded = EmoteStatus::Loaded;
    };

    if let Ok(event) = self.emote_loader.as_mut().unwrap().rx.try_recv() {
      match event {
        EmoteResponse::GlobalEmoteImageLoaded { name, data } => {
          if let Some(emote) = self.global_emotes.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data);
          }
        },
        EmoteResponse::GlobalBadgeImageLoaded { name, data } => {
          if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) 
          && let Some(global_badges) = &mut provider.global_badges && let Some(emote) = global_badges.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data);
          }
        },
        EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(emote) = channel.transient.as_mut()
          .and_then(|t| t.channel_emotes.as_mut()).and_then(|f| { f.get_mut(&name)}) {
            set_emote_texture_data(emote, ctx, data);
          }
        },
        EmoteResponse::ChannelBadgeImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(emote) = channel.transient.as_mut()
          .and_then(|t| t.badge_emotes.as_mut()).and_then(|f| { f.get_mut(&name)}) {
            set_emote_texture_data(emote, ctx, data);
          }
        },
        EmoteResponse::TwitchMsgEmoteLoaded { name, id: _, data } => {
          if let Some(p) = self.providers.get_mut(&ProviderName::Twitch) && let Some(emote) = p.emotes.get_mut(&name) {
            set_emote_texture_data(emote, ctx, data);
          }
        }
      }
    }

    let mut channel_swap = false;
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

    let mut add_channel = |providers: &mut HashMap<ProviderName, Provider>, auth_tokens: &mut AuthTokens, channel_options: &mut AddChannelMenu, emote_loader : &EmoteLoader| {
      let c = match channel_options.provider {
        ProviderName::Twitch => { 
          providers.entry(ProviderName::Twitch).or_insert(Provider {
              name: "twitch".to_owned(),
              my_sub_emotes: Default::default(),
              emotes: Default::default(),
              global_badges: emote_loader.twitch_get_global_badges(&auth_tokens.twitch_auth_token)
          });
          if self.twitch_chat_manager.is_none() {
            self.twitch_chat_manager = Some(TwitchChatManager::new(&auth_tokens.twitch_username, &auth_tokens.twitch_auth_token, self.runtime.as_ref().unwrap()));
          }
          self.twitch_chat_manager.as_mut().unwrap().init_channel(&channel_options.channel_name)
          //twitch::init_channel(&auth_tokens.twitch_username, &auth_tokens.twitch_auth_token, channel_options.channel_name.to_owned(), self.runtime.as_ref().unwrap(), emote_loader)
        },
        ProviderName::DGG => dgg::init_channel()
        /*ProviderName::YouTube => {
          if providers.contains_key(&ProviderName::Twitch) == false {
            providers.insert(ProviderName::Twitch, Provider {
                name: "youtube".to_owned(),
                my_sub_emotes: Default::default(),
                emotes: Default::default(),
                global_badges: Default::default()
            });
          }
          youtube::init_channel(channel_options.channel_name.to_owned(), channel_options.channel_id.to_owned(), auth_tokens.youtube_auth_token.to_owned(), self.runtime.as_ref().unwrap())
        }*/
      };

      let name = c.channel_name.to_owned();
      self.channels.insert(name.to_owned(), c);
      self.selected_channel = Some(name);
      channel_options.channel_name = Default::default();
    };

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
            twitch::authenticate(ctx, self.runtime.as_ref().unwrap());
          }
        });
        ui.separator();
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
            self.auth_tokens.dgg_verifier = dgg::begin_authenticate(ctx);
          }
        });
        /*ui.horizontal(|ui| {
          ui.label("YouTube");
          ui.text_edit_singleline(&mut self.auth_tokens.youtube_auth_token);
        });
        ui.separator();*/
        if ui.button("Ok").clicked() {
          let twitch_token = self.auth_tokens.twitch_auth_token.to_owned();
          if twitch_token.starts_with('#') || twitch_token.starts_with("access") {
            let rgx = regex::Regex::new("access_token=(.*?)&").unwrap();
            let cleaned = rgx.captures(twitch_token.as_str()).unwrap().get(1).map_or("", |x| x.as_str());
            self.auth_tokens.twitch_auth_token = cleaned.to_owned();
            if !cleaned.is_empty() {
              self.auth_tokens.show_twitch_auth_token = false;
            }
          }
          let dgg_token = self.auth_tokens.dgg_auth_token.to_owned();
          if dgg_token.starts_with('?') || dgg_token.starts_with("code") {
            let rgx = regex::Regex::new("code=(.*?)&").unwrap();
            let cleaned = rgx.captures(dgg_token.as_str()).unwrap().get(1).map_or("", |x| x.as_str());
            if !cleaned.is_empty() {
              let token = dgg::complete_authenticate(cleaned, &self.auth_tokens.dgg_verifier);
              self.auth_tokens.dgg_auth_token = token.expect("failed to get dgg token");
              self.auth_tokens.dgg_verifier = Default::default();
              self.auth_tokens.show_dgg_auth_token = false;
            }
          }
          self.show_auth_ui = false;
        }
      }).unwrap();
      if ctx.input().pointer.any_click() 
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && !auth_menu.response.rect.contains(pos) {
        self.show_auth_ui = false;
      }
      else if ctx.input().key_pressed(Key::Escape) {
        self.show_auth_ui = false;
      }
    }

    if self.show_add_channel_menu {
      let add_menu = egui::Window::new("Add Channel").collapsible(false).show(ctx, |ui| {
        let mut name_input : Option<egui::Response> = None;
        ui.horizontal(|ui| {
          ui.label("Provider:");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::Twitch, "Twitch");
          //ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::YouTube, "Youtube");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::DGG, "destiny.gg");
        });
        ui.horizontal(|ui| {
          ui.label("Channel Name:");
          name_input = Some(ui.text_edit_singleline(&mut self.add_channel_menu.channel_name));
          //name_input.request_focus();
          
        });
        /*if self.add_channel_menu.provider == ProviderName::YouTube {
          ui.horizontal(|ui| {
            ui.label("Channel ID:");
            ui.text_edit_singleline(&mut self.add_channel_menu.channel_id);
          });
        }*/
        
        if name_input.unwrap().has_focus() && ui.input().key_pressed(egui::Key::Enter) || ui.button("Add channel").clicked() {
          add_channel(&mut self.providers, &mut self.auth_tokens, &mut self.add_channel_menu, self.emote_loader.as_mut().unwrap()); 
          self.show_add_channel_menu = false;
        }
        if ui.button("Cancel").clicked() {
          self.show_add_channel_menu = false;
        }
      }).unwrap();
      if ctx.input().pointer.any_click() 
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && !add_menu.response.rect.contains(pos) {
        self.show_add_channel_menu = false;
      }
      else if ctx.input().key_pressed(Key::Escape) {
        self.show_add_channel_menu = false;
      }
    }

    
    let mut channel_removed = false;
    if self.show_channel_options {
      let channels = self.channels.iter_mut();
      let add_menu = egui::Window::new(format!("Configure Channel: {}", self.selected_channel.as_ref().unwrap_or(&"".to_owned()))).collapsible(false).show(ctx, |ui| {
        if let Some(channel) = self.selected_channel.as_ref() {
          if ui.button("Remove channel").clicked() {
            if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
              chat_mgr.leave_channel(channel);
              channel_removed = true;
              self.show_channel_options = false;
            }
          }
        } else {
          for (name, channel) in channels {
            ui.checkbox(&mut channel.show_in_all, name);
          }
        }
      }).unwrap();
      if ctx.input().pointer.any_click() 
          && let Some(pos) = ctx.input().pointer.interact_pos() 
          && !add_menu.response.rect.contains(pos) {
        self.show_channel_options = false;
      }
      else if ctx.input().key_pressed(Key::Escape) {
        self.show_channel_options = false;
      }
    }

    if self.twitch_chat_manager.is_none() {
      self.twitch_chat_manager = Some(TwitchChatManager::new(&self.auth_tokens.twitch_username, &self.auth_tokens.twitch_auth_token, self.runtime.as_ref().unwrap()));
    }
    if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() && let Ok(x) = chat_mgr.out_rx.try_recv() {
      self.handle_incoming_message(x);
    }
    if let Some(chat_mgr) = self.dgg_chat_manager.as_mut() && let Ok(x) = chat_mgr.out_rx.try_recv() {
      self.handle_incoming_message(x);
    }
    if self.dgg_chat_manager.is_none() && let Some((_, sco)) = self.channels.iter_mut().find(|f| f.1.provider == ProviderName::DGG) {
      self.dgg_chat_manager = Some(dgg::open_channel(&"NullGex".to_owned(), &self.auth_tokens.dgg_auth_token, sco, self.runtime.as_ref().unwrap(), self.emote_loader.as_ref().unwrap()));
    }

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      ui.horizontal(|ui| {
        egui::menu::bar(ui, |ui| {
          if ui.menu_button(RichText::new("Add a channel").size(SMALL_TEXT_SIZE), |ui| { ui.close_menu(); }).response.clicked() {
            self.show_add_channel_menu = true;
          }
          ui.separator();
          if ui.menu_button(RichText::new("Configure Tokens").size(SMALL_TEXT_SIZE), |ui| { ui.close_menu(); }).response.clicked() {
            self.show_auth_ui = true;
          }
          ui.separator();
          ui.menu_button(RichText::new("Options").size(SMALL_TEXT_SIZE), |ui| {
            ui.checkbox(&mut self.enable_combos, "Enable Combos");
          });
          ui.separator();
          if ui.menu_button(RichText::new("View on Github").size(SMALL_TEXT_SIZE), |ui| { ui.close_menu(); }).response.clicked() {
            _ = ctx.output().open_url("https://github.com/dbckr/gigachat");
          }
        });
      });
      ui.separator();

      ui.horizontal_wrapped(|ui| {
        let label = RichText::new("All Channels").size(BUTTON_TEXT_SIZE);
        let clbl = ui.selectable_value(&mut self.selected_channel, None, label);
        if clbl.clicked() {
          channel_swap = true;
        }
        else if clbl.clicked_by(egui::PointerButton::Secondary) {
          self.show_channel_options = true;
        }

        let channels = &mut self.channels;
        for (channel, sco) in channels.iter_mut() {  
          if let Some(t) = sco.transient.as_mut() {            
            let mut job = LayoutJob { ..Default::default() };
            job.append(channel, 0., egui::TextFormat {
              font_id: FontId::new(BUTTON_TEXT_SIZE, FontFamily::Proportional), 
              color: Color32::LIGHT_GRAY,
              ..Default::default()
            });
            if t.status.is_some_and(|s| s.is_live) {
              let red = if self.selected_channel.as_ref() == Some(&sco.channel_name) { 255 } else { 220 };
              job.append("🔴", 3., egui::TextFormat {
                font_id: FontId::new(BUTTON_TEXT_SIZE / 1.5, FontFamily::Proportional), 
                color: Color32::from_rgb(red, 0, 0),
                valign: Align::Center,
                ..Default::default()
              });
            }
            let clbl = ui.selectable_value(&mut self.selected_channel, Some(channel.to_owned()), job);
            if clbl.clicked() {
              channel_swap = true;
            }
            else if clbl.clicked_by(egui::PointerButton::Secondary) {
              self.show_channel_options = true;
            }
            else if clbl.middle_clicked() && let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
              chat_mgr.leave_channel(channel);
              channel_removed = true;
            }
            if let Some(status) = &t.status && status.is_live {
              clbl.on_hover_ui(|ui| {
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
              });
            }
          }
          else if sco.provider == ProviderName::Twitch && let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
            // channel has not been opened yet
            chat_mgr.open_channel(sco);
          }
        }
      });
    });
    if channel_removed {
      if let Some(name) = &self.selected_channel {
        self.channels.remove(name);
      }
      self.selected_channel = None;
    }

    let cframe = egui::Frame { 
      inner_margin: egui::style::Margin::same(5.0), 
      fill: egui::Color32::from_rgba_unmultiplied(50, 50, 50, 50),
      ..Default::default() 
    };
    
    egui::CentralPanel::default().frame(cframe).show(ctx, |ui| {
      ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
        if let Some(sc) = &self.selected_channel.to_owned() {
          let goto_next_emote = self.selected_emote.is_some() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowRight);
          let goto_prev_emote = self.selected_emote.is_some() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowLeft);
          let enter_emote = self.selected_emote.is_some() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowDown);
          let prev_history = ui.input_mut().consume_key(Modifiers::NONE, Key::ArrowUp);
          let next_history = ui.input_mut().consume_key(Modifiers::NONE, Key::ArrowDown);

          ui.style_mut().visuals.extreme_bg_color = Color32::from_rgba_premultiplied(0, 0, 0, 120);
          let mut outgoing_msg = egui::TextEdit::multiline(&mut self.draft_message)
            .desired_rows(2)
            .desired_width(ui.available_width())
            .hint_text("Type a message to send")
            .font(egui::TextStyle::Body)
            .show(ui);  

          if prev_history || next_history {
            if let Some(sco) = (&mut self.channels).get_mut(sc) {
              let mut ix = sco.send_history_ix.unwrap_or(0);
              let msg = sco.send_history.get(ix);
              if prev_history {
                ix = ix.add(1).min(sco.send_history.len() - 1);
              } else {
                ix = ix.saturating_sub(1);
              };
              if let Some(msg) = msg {
                self.draft_message = msg.to_owned();
                outgoing_msg.state.set_ccursor_range(
                  Some(egui::text_edit::CCursorRange::one(egui::text::CCursor::new(self.draft_message.len())))
                );
              }
              sco.send_history_ix = Some(ix);
            }
          }

          if outgoing_msg.response.has_focus() && ui.input().key_down(egui::Key::Enter) && !ui.input().modifiers.shift && !self.draft_message.is_empty() {
            if let Some(sco) = (&mut self.channels).get_mut(sc) {
              if sco.provider == ProviderName::Twitch && let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
                match chat_mgr.in_tx.try_send(OutgoingMessage::Chat { channel_name: sco.channel_name.to_owned(), message: self.draft_message.replace('\n', " ") }) {
                  Err(e) => println!("Failed to send message: {}", e), //TODO: emit this into UI
                  _ => {
                    sco.send_history.push_front(self.draft_message.to_owned());
                    self.draft_message = String::new();
                  }
                }
              } 
              else if sco.provider == ProviderName::DGG && let Some(chat_mgr) = self.dgg_chat_manager.as_mut() {
                match chat_mgr.in_tx.try_send(OutgoingMessage::Chat { channel_name: "".to_owned(), message: self.draft_message.replace('\n', " ") }) {
                  Err(e) => println!("Failed to send message: {}", e), //TODO: emit this into UI
                  _ => {
                    sco.send_history.push_front(self.draft_message.to_owned());
                    self.draft_message = String::new();
                  }
                }
              }
            } 
          }
          else if !self.draft_message.is_empty() && let Some(cursor_pos) = outgoing_msg.state.ccursor_range() {
            let cursor = cursor_pos.primary.index;
            let emotes = self.get_possible_emotes(cursor);
            if let Some((word, pos, emotes)) = emotes && !emotes.is_empty() {
              if enter_emote && let Some(emote_text) = &self.selected_emote {
                let msg = if self.draft_message.len() <= pos + word.len() || &self.draft_message[pos + word.len()..pos + word.len() + 1] != " " {
                  format!("{}{} {}", &self.draft_message[..pos], emote_text, &self.draft_message[pos + word.len()..])
                } else {
                  format!("{}{}{}", &self.draft_message[..pos], emote_text, &self.draft_message[pos + word.len()..])
                };
                self.draft_message = msg;
                outgoing_msg.response.request_focus();
                println!("{}", emote_text.len());
                outgoing_msg.state.set_ccursor_range(
                  Some(egui::text_edit::CCursorRange::one(egui::text::CCursor::new(self.draft_message[..pos].len() + emote_text.len() + 1)))
                );
                self.selected_emote = None;
              }
              else {
                if goto_next_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == self.selected_emote.as_ref()) && ix + 1 < emotes.len() {
                  self.selected_emote = emotes.get(ix + 1).map(|x| x.0.to_owned());
                }
                else if goto_prev_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == self.selected_emote.as_ref()) && ix > 0 {
                  self.selected_emote = emotes.get(ix - 1).map(|x| x.0.to_owned());
                }
                else if self.selected_emote.is_none() || !emotes.iter().any(|x| Some(&x.0) == self.selected_emote.as_ref()) {
                  self.selected_emote = emotes.first().map(|x| x.0.to_owned());
                }

                // Overlay style emote selector
                let msg_rect = outgoing_msg.response.rect.to_owned();
                let ovl_height = ui.available_height() / 4.;
                let painter_rect = msg_rect.expand2(egui::vec2(0., ovl_height)).translate(egui::vec2(0., (msg_rect.height() + ovl_height + 8.) * -1.));
                let mut painter = ui.painter_at(painter_rect);
                let painter_rect = painter.clip_rect();
                painter.set_layer_id(egui::LayerId::debug());

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

                  let texture = emote.1.as_ref()
                    .and_then(|f| f.texture.as_ref())
                    .or_else(|| self.emote_loader.as_ref().unwrap().transparent_img.as_ref())
                    .unwrap();

                  let width = texture.size_vec2().x * (EMOTE_HEIGHT / texture.size_vec2().y);
                  if x + width + text_width > painter_rect.right() {
                    y -= EMOTE_HEIGHT;
                    x = painter_rect.left();
                  }

                  let uv = egui::Rect::from_two_pos(egui::pos2(0., 0.), egui::pos2(1., 1.));
                  let rect = egui::Rect { 
                    min: egui::pos2(painter_rect.left() + x, y - EMOTE_HEIGHT ), 
                    max: egui::pos2(painter_rect.left() + x + width, y) 
                  };

                  painter.rect_filled(egui::Rect {
                    min: egui::pos2(x, y - EMOTE_HEIGHT),
                    max: egui::pos2(x + width + text_width, y),
                  }, Rounding::none(), Color32::from_rgba_unmultiplied(0, 0, 0, 210));

                  let mut mesh = egui::Mesh::with_texture(texture.id());
                  mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
                  painter.add(egui::Shape::mesh(mesh));

                  let disp_text = emote.0.to_owned();
                  painter.text(egui::pos2(painter_rect.left() + x + width, y), egui::Align2::LEFT_BOTTOM, disp_text, FontId::new(BODY_TEXT_SIZE, egui::FontFamily::Proportional), if self.selected_emote == Some(emote.0) { Color32::RED } else { Color32::WHITE });

                  x = x + width + text_width;
                }
              }
            }
          }

          outgoing_msg.state.store(ctx, outgoing_msg.response.id);
        }
        
        let mut popped_height = 0.;
        for (_channel, history) in self.chat_histories.iter_mut() {
          if history.len() > 2000 && let Some(popped) = history.pop_front() 
            && let Some(height) = popped.1 && (self.selected_channel.is_none() || self.selected_channel == Some(popped.0.channel)) {
            if self.enable_combos && popped.0.combo_data.is_some_and(|c| !c.is_end) {
              // add nothing to y_pos
            } else if self.enable_combos && popped.0.combo_data.is_some_and(|c| c.is_end && c.count > 1) {
              popped_height += COMBO_LINE_HEIGHT + ui.spacing().item_spacing.y;
            } else {
              popped_height += height;
            }
          }
        }

        if channel_swap {
          self.chat_scroll = None;
        }

        ui.style_mut().visuals.override_text_color = Some(egui::Color32::LIGHT_GRAY);
        let chat_area = egui::ScrollArea::vertical()
          .auto_shrink([false; 2])
          .stick_to_bottom()
          .always_show_scroll(true)
          .scroll_offset(self.chat_scroll.map(|f| egui::Vec2 {x: 0., y: f.y - popped_height }).unwrap_or(egui::Vec2 {x: 0., y: 0.}));
        let area = chat_area.show_viewport(ui, |ui, viewport| {
          self.show_variable_height_rows(ui, viewport, &self.selected_channel.to_owned());
        });
        // if stuck to bottom, y offset at this point should be equal to scrollarea max_height - viewport height
        self.chat_scroll = Some(area.state.offset);
      });
    });

    //std::thread::sleep(Duration::from_millis(10));
    ctx.request_repaint();
  }

  fn show_variable_height_rows(&mut self, ui : &mut egui::Ui, viewport: Rect, _channel_name: &Option<String>) {
    ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
      ui.spacing_mut().item_spacing.x = 4.0;
      //ui.spacing_mut().item_spacing.y = 1.;

      let y_min = ui.max_rect().top() + viewport.min.y;
      let y_max = ui.max_rect().top() + viewport.max.y;
      let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
      let mut in_view : Vec<UiChatMessage> = Default::default();
      let mut y_pos = 0.0;
      let mut excess_top_space : Option<f32> = None;
      let mut skipped_rows = 0;
      
      let mut history_iters = Vec::new();
      for (cname, hist) in self.chat_histories.iter_mut() {
        if self.selected_channel.is_some_and(|channel| channel == cname) 
          || self.selected_channel.is_none() && self.channels.get(cname).is_some_and(|f| f.show_in_all) {
          history_iters.push(hist.iter_mut().peekable());
        }
      }
      let mut history_iters = HistoryIterator {
        iterators: history_iters,
      };
      let show_channel_names = history_iters.iterators.len() > 1;

      while let Some((row, cached_y)) = history_iters.get_next() {
        let combo = &row.combo_data;

        // Skip processing if row size is accurately cached and not in view
        if let Some(last_viewport) = self.chat_frame && last_viewport.size() == viewport.size() && let Some(size_y) = cached_y.as_ref()
          && (y_pos < viewport.min.y - 1000. || y_pos + size_y > viewport.max.y + excess_top_space.unwrap_or(0.) + 1000.) {
            if self.enable_combos && combo.is_some_and(|c| !c.is_end) {
              // add nothing to y_pos
            } else if self.enable_combos && combo.is_some_and(|c| c.is_end && c.count > 1) {
              y_pos += COMBO_LINE_HEIGHT + ui.spacing().item_spacing.y;
            } else {
              y_pos += size_y;
            }
            if y_pos < viewport.min.y - 200. {
              skipped_rows += 1;
            }
            continue;
        }

        let (provider_emotes, provider_badges) = self.providers.get_mut(&ProviderName::Twitch)
          .map(|p| (Some(&mut p.emotes), p.global_badges.as_mut())).unwrap_or((None, None));
        let (channel_emotes, channel_badges) = self.channels.get_mut(&row.channel)
          .and_then(|c| c.transient.as_mut())
          .map(|t| (t.channel_emotes.as_mut(), t.badge_emotes.as_mut())).unwrap_or((None, None));
        let emotes = get_emotes_for_message(row, provider_emotes, channel_emotes, &mut self.global_emotes, self.emote_loader.as_mut().unwrap());
        let (badges, user_color) = get_badges_for_message(row.profile.badges.as_ref(), &row.channel, provider_badges, channel_badges, self.emote_loader.as_mut().unwrap());
        let (msg_sizing, is_ascii_art) = chat_estimate::get_chat_msg_size(ui, row, &emotes, badges.as_ref(), show_channel_names);

        // DGG user colors are tied to badge/flair
        if row.profile.color.is_none() && user_color.is_some() {
          row.profile.color = user_color;
        }

        let mut lines_to_include : Vec<UiChatMessageRow> = Default::default();
        let mut row_y = 0.;
        for line in msg_sizing {
          let size_y = line.0;
          //println!("{} {}", viewport.min.y, viewport.max.y);
          if y_pos + row_y >= viewport.min.y && y_pos + row_y + size_y <= viewport.max.y + excess_top_space.unwrap_or(0.) {
            if excess_top_space.is_none() {
              excess_top_space = Some(y_pos + row_y - viewport.min.y);
            }
            lines_to_include.push(UiChatMessageRow { row_height: line.0, msg_char_range: line.1, is_visible: true });
          } 
          else {
            lines_to_include.push(UiChatMessageRow { row_height: line.0, msg_char_range: line.1, is_visible: false });
          }
          row_y += size_y + match is_ascii_art { true => 0., false => ui.spacing().item_spacing.y };
        }
        if self.enable_combos && combo.is_some_and(|c| !c.is_end) {
          // add nothing to y_pos
        } else if self.enable_combos && combo.is_some_and(|c| c.is_end && c.count > 1) {
          y_pos += COMBO_LINE_HEIGHT + ui.spacing().item_spacing.y;
        } else {
          y_pos += row_y;
        }
        *cached_y = Some(row_y);

        if (&lines_to_include).iter().any(|x| x.is_visible) {
          //in_view.push((row, emotes, badges, msg_sizing, lines_to_include, row_y, finished_combo.or(Some(combo.clone()))));
          in_view.push(UiChatMessage {
            message: row,
            emotes,
            badges,
            row_data: lines_to_include,
            msg_height: row_y,
            is_ascii_art
          });
        }
      }

      let transparent_texture = self.emote_loader.as_ref().unwrap().transparent_img.as_ref().unwrap();
      self.chat_frame = Some(viewport.to_owned());
      ui.set_height(y_pos);
      ui.skip_ahead_auto_ids(skipped_rows);
      //if *is_swap {
      //  ui.scroll_to_rect(Rect::from_min_size(Pos2 { x: 0., y: 0. }, Vec2 { x: 1., y: 1. }), None);
      //}
      ui.allocate_ui_at_rect(rect, |viewport_ui| {
        for chat_msg in in_view.iter() {
          if !self.enable_combos || chat_msg.message.combo_data.is_none() || chat_msg.message.combo_data.is_some_and(|c| c.is_end && c.count == 1) {
            chat::create_chat_message(viewport_ui, chat_msg, transparent_texture, show_channel_names);
          }
          else if chat_msg.message.combo_data.as_ref().is_some_and(|combo| combo.is_end) { 
            chat::create_combo_message(viewport_ui, chat_msg, transparent_texture, show_channel_names);
          }
        }
      });
    });
  }

  fn handle_incoming_message(&mut self, x: IncomingMessage) {
    match x {
      IncomingMessage::PrivMsg { mut message } => {
        let provider_emotes = self.providers.get_mut(&message.provider).map(|f| &mut f.emotes);
        let channel = message.channel.to_owned();
        // remove any extra whitespace between words
        let rgx = regex::Regex::new("\\s+").unwrap();
        message.message = rgx.replace_all(&message.message, " ").to_string();
        let chat_history = self.chat_histories.entry(channel.to_owned()).or_insert_with(Default::default);
        push_history(
          chat_history, 
          message,
          provider_emotes, 
          self.channels.get_mut(&channel).and_then(|f| f.transient.as_mut()).and_then(|f| f.channel_emotes.as_mut()),
          &mut self.global_emotes, 
          self.emote_loader.as_mut().unwrap());
      },
      IncomingMessage::StreamingStatus { channel, status } => {
        if let Some(t) = self.channels.get_mut(&channel).and_then(|f| f.transient.as_mut()) {
          t.status = status;
        }
      },
      IncomingMessage::MsgEmotes { provider, emote_ids } => {
        if let Some(provider) = self.providers.get_mut(&provider) {
          for (id, name) in emote_ids {
            if !provider.emotes.contains_key(&name) {
              provider.emotes.insert(name.to_owned(), Emote { name, id, url: "".to_owned(), path: "cache/twitch/".to_owned(), ..Default::default() });
            }
          }
        }
      },
      IncomingMessage::RoomId { channel, room_id } => {
        if let Some(sco) = self.channels.get_mut(&channel) && let Some(t) = sco.transient.as_mut() {
          sco.roomid = room_id;
          match self.emote_loader.as_mut().unwrap().load_channel_emotes(&sco.roomid, match &sco.provider {
            ProviderName::Twitch => &self.auth_tokens.twitch_auth_token,
            ProviderName::DGG => &self.auth_tokens.dgg_auth_token
            //ProviderName::YouTube => &self.auth_tokens.youtube_auth_token
          }) {
            Ok(x) => {
              t.channel_emotes = Some(x);
            },
            Err(x) => { 
              println!("ERROR LOADING CHANNEL EMOTES: {}", x); 
              Default::default()
            }
          };
          t.badge_emotes = self.emote_loader.as_mut().unwrap().twitch_get_channel_badges(&self.auth_tokens.twitch_auth_token, &sco.roomid);
          println!("loaded channel badges for {}:{}", channel, sco.roomid);
          //break;
        }
      },
      IncomingMessage::EmoteSets { provider,  emote_sets } => {
        if let Some(provider) = self.providers.get_mut(&provider) {
          for set in emote_sets {
            if let Some(set_list) = self.emote_loader.as_mut().unwrap().twitch_get_emote_set(&self.auth_tokens.twitch_auth_token, &set) {
              for (_id, emote) in set_list {
                provider.my_sub_emotes.insert(emote.name.to_owned());
                if !provider.emotes.contains_key(&emote.name) {
                  provider.emotes.insert(emote.name.to_owned(), emote);
                }
              }
            }
          }
      
        }
      }
    };
  }

  fn get_possible_emotes(&mut self, cursor_position: usize) -> Option<(String, usize, Vec<(String, Option<EmoteFrame>)>)> {
    let msg = &self.draft_message;
    let word : Option<(usize, &str)> = msg.split_whitespace()
      .map(move |s| (s.as_ptr() as usize - msg.as_ptr() as usize, s))
      .filter_map(|p| if p.0 <= cursor_position && cursor_position <= p.0 + p.1.len() { Some((p.0, p.1)) } else { None })
      .next();

    if let Some((pos, input_str)) = word {
      if input_str.len() < 2  {
        return None;
      }
      let word = &input_str[0..];
      let word_lower = &word.to_lowercase();

      let mut starts_with_emotes : HashMap<String, Option<EmoteFrame>> = Default::default();
      let mut contains_emotes : HashMap<String, Option<EmoteFrame>> = Default::default();
      // Find similar emotes. Show emotes starting with same string first, then any that contain the string.
      if let Some(channel_name) = &self.selected_channel && let Some(channel) = self.channels.get_mut(channel_name) {
          if let Some(transient) = channel.transient.as_mut() && let Some(channel_emotes) = transient.channel_emotes.as_mut() {
          for (name, emote) in channel_emotes { // Channel emotes
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              let tex = chat::get_texture(self.emote_loader.as_mut().unwrap(), emote, EmoteRequest::new_channel_request(emote, channel_name));
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
                let tex = chat::get_texture(self.emote_loader.as_mut().unwrap(), emote, EmoteRequest::new_twitch_emote_request(emote));
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
              let tex = chat::get_texture(self.emote_loader.as_mut().unwrap(), emote, EmoteRequest::new_global_request(emote));
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
}

fn push_history(chat_history: &mut VecDeque<(ChatMessage, Option<f32>)>, mut message: ChatMessage, provider_emotes: Option<&mut HashMap<String, Emote>>, channel_emotes: Option<&mut HashMap<String, Emote>>, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) {
  let is_emote = !get_emotes_for_message(&message, provider_emotes, channel_emotes, global_emotes, emote_loader).is_empty();
  let last = chat_history.iter_mut().rev().find_or_first(|f| f.0.channel == message.channel);
  if let Some(last) = last && is_emote {
    let combo = combo_calculator(&message, last.0.combo_data.as_ref());
    if combo.is_some_and(|c| !c.is_new && c.count > 1) && let Some(last_combo) = last.0.combo_data.as_mut() {
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

fn get_emotes_for_message(row: &ChatMessage, provider_emotes: Option<&mut HashMap<String, Emote>>, channel_emotes: Option<&mut HashMap<String, Emote>>, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> HashMap<String, EmoteFrame> {
  let mut result : HashMap<String, chat::EmoteFrame> = Default::default();
  for word in row.message.to_owned().split(' ') {
    let emote = 
      if let Some(&mut ref mut channel_emotes) = channel_emotes && let Some(emote) = channel_emotes.get_mut(word) {
        Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_request(emote, &row.channel)))
      }
      else if row.provider != ProviderName::DGG && let Some(emote) = global_emotes.get_mut(word) {
        Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_global_request(emote)))
      }
      else if let Some(&mut ref mut provider_emotes) = provider_emotes && let Some(emote) = provider_emotes.get_mut(word) {
        Some(chat::get_texture(emote_loader, emote, EmoteRequest::new_twitch_emote_request(emote)))
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

fn get_badges_for_message(badges: Option<&Vec<String>>, channel_name: &str, global_badges: Option<&mut HashMap<String, Emote>>, channel_badges: Option<&mut HashMap<String, Emote>>, emote_loader: &mut EmoteLoader) -> (Option<HashMap<String, EmoteFrame>>, Option<(u8,u8,u8)>) {
  let mut result : HashMap<String, chat::EmoteFrame> = Default::default();
  if badges.is_none() { return (None, None); }
  let mut greatest_badge : Option<(isize, (u8,u8, u8))> = None;
  for badge in badges.unwrap() {
    let emote = 
      if let Some(&mut ref mut channel_badges) = channel_badges && let Some(emote) = channel_badges.get_mut(badge) {
        if channel_name == dgg::DGG_CHANNEL_NAME && emote.color.is_some() && (greatest_badge.is_none() || greatest_badge.is_some_and(|b| b.0 < emote.priority)) {
          greatest_badge = Some((emote.priority, emote.color.unwrap()))
        }
        chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_badge_request(emote, channel_name))
      }
      else if let Some(&mut ref mut global_badges) = global_badges && let Some(emote) = global_badges.get_mut(badge) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_global_badge_request(emote))
      }
      else {
        EmoteFrame { id: badge.to_owned(), name: badge.to_owned(), label: None, path: badge.to_owned(), texture: None, zero_width: false }
      };
    
    result.insert(emote.name.to_owned(), emote);
  }

  (Some(result), greatest_badge.map(|x| x.1))
}

pub fn load_font() -> FontDefinitions {
  let mut fonts = FontDefinitions::default();

  let font_file = load_file_into_buffer("C:\\Windows\\Fonts\\segoeui.ttf");
  let font = FontData::from_owned(font_file);

  let symbols_font = load_file_into_buffer("C:\\Windows\\Fonts\\seguisym.ttf");
  let symbols = FontData::from_owned(symbols_font);

  let emojis_font = load_file_into_buffer("C:\\Windows\\Fonts\\seguiemj.ttf");
  let emojis = FontData::from_owned(emojis_font);

  fonts.font_data.insert("def_font".into(), font);
  fonts.font_data.insert("symbols".into(), symbols);
  fonts.font_data.insert("emojis".into(), emojis);

  fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "def_font".into());
  fonts.families.entry(FontFamily::Monospace).or_default().push("def_font".into());

  fonts.families.entry(FontFamily::Proportional).or_default().push("symbols".into());
  fonts.families.entry(FontFamily::Monospace).or_default().push("symbols".into());

  fonts.families.entry(FontFamily::Proportional).or_default().push("emojis".into());
  fonts.families.entry(FontFamily::Monospace).or_default().push("emojis".into());

  fonts
}

struct HistoryIterator<'a> {
  //histories: Vec<VecDeque<(ChatMessage, Option<f32>)>>,
  iterators: Vec<Peekable<IterMut<'a, (ChatMessage, Option<f32>)>>>,
}

impl<'a> HistoryIterator<'a> {
  fn get_next(&mut self) -> Option<&'a mut (ChatMessage, Option<f32>)> {
    let mut min_i = 0;
    let mut ts = Utc::now();

    let mut i = 0;
    for iter in self.iterators.iter_mut() {
      if let Some((msg, _y)) = iter.peek() && msg.timestamp < ts {
        ts = msg.timestamp;
        min_i = i;
      }
      i += 1;
    }

    self.iterators.get_mut(min_i).and_then(|x| x.next())
  }
}