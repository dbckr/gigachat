/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::{HashMap, VecDeque}};
use eframe::{egui::{self, emath, RichText, Key, Modifiers}, epi, epaint::{FontId}, emath::{Align, Rect}};
use egui::Vec2;
use image::DynamicImage;
use itertools::Itertools;
use crate::{provider::{twitch, ChatMessage, InternalMessage, OutgoingMessage, Channel, Provider, ProviderName, youtube}};
use crate::{emotes, emotes::{Emote, EmoteLoader, EmoteStatus, EmoteRequest, EmoteResponse, imaging::{load_image_into_texture_handle, load_to_texture_handles}}};
use self::chat::EmoteFrame;

pub mod chat;
pub mod chat_estimate;

const BUTTON_TEXT_SIZE : f32 = 18.0;
const BODY_TEXT_SIZE : f32 = 16.0;
const SMALL_TEXT_SIZE : f32 = 13.0;
/// Max length before manually splitting up a string without whitespace
const WORD_LENGTH_MAX : usize = 40;
/// Emotes in chat messages will be scaled to this height
pub const EMOTE_HEIGHT : f32 = 26.0;
const BADGE_HEIGHT : f32 = 18.0;
/// Should be at least equal to ui.spacing().interact_size.y
const MIN_LINE_HEIGHT : f32 = 21.0;

pub struct AddChannelMenu {
  channel_name: String,
  channel_id: String,
  provider: ProviderName,
}

impl Default for AddChannelMenu {
    fn default() -> Self {
        Self { 
          channel_name: Default::default(), 
          channel_id: Default::default(), 
          provider: ProviderName::Twitch }
    }
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AuthTokens {
  pub twitch_username: String,
  pub twitch_auth_token: String,
  pub youtube_auth_token: String
}

#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))]
pub struct TemplateApp {
  #[cfg_attr(feature = "persistence", serde(skip))]
  runtime: Option<tokio::runtime::Runtime>,
  pub providers: HashMap<ProviderName, Provider>,
  channels: HashMap<String, Channel>,
  selected_channel: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  chat_history: VecDeque<(ChatMessage, Option<f32>)>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  draft_message: String,
  #[cfg_attr(feature = "persistence", serde(skip))]
  add_channel_menu_show: bool,
  #[cfg_attr(feature = "persistence", serde(skip))]
  add_channel_menu: AddChannelMenu,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub global_emotes: HashMap<String, Emote>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub emote_loader: Option<EmoteLoader>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  pub selected_emote: Option<String>,
  #[cfg_attr(feature = "persistence", serde(skip))]
  show_auth_ui: bool,
  pub auth_tokens: AuthTokens,
  chat_frame: Option<Rect>,
  chat_scroll: Option<Vec2>,
}


impl TemplateApp {
  pub fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Runtime) -> Self {
      cc.egui_ctx.set_visuals(egui::Visuals::dark());
      let mut r = TemplateApp {
        ..Default::default()
      };
      #[cfg(feature = "persistence")]
      if let Some(storage) = cc.storage {
          r = epi::get_value(storage, epi::APP_KEY).unwrap_or_default();
      }
      let mut loader = EmoteLoader::new(&runtime);
      loader.transparent_img = Some(load_image_into_texture_handle(&cc.egui_ctx, DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 255]) ))));
      r.runtime = Some(runtime);
      r.emote_loader = Some(loader);
      r
  }
}

impl epi::App for TemplateApp {
  #[cfg(feature = "persistence")]
  fn save(&mut self, storage: &mut dyn epi::Storage) {
    epi::set_value(storage, epi::APP_KEY, self);
  }

  fn update(&mut self, ctx: &egui::Context, frame: &mut epi::Frame) {
    if ctx.pixels_per_point() == 1.75 {
      ctx.set_pixels_per_point(1.50);
    }

    let set_emote_texture_data = |emote: &mut Emote, ctx: &egui::Context, data: Option<Vec<(DynamicImage, u16)>>| {
      emote.data = load_to_texture_handles(ctx, data);
      emote.duration_msec = match emote.data.as_ref() {
        Some(framedata) => framedata.iter().map(|(_, delay)| delay).sum(),
        _ => 0,
      };
      emote.loaded = EmoteStatus::Loaded;
    };

    while let Ok(event) = self.emote_loader.as_mut().unwrap().rx.try_recv() {
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
        EmoteResponse::EmoteSetImageLoaded { name, set_id, provider_name, data } => {
          if let Some(provider) = self.providers.get_mut(&provider_name) 
          && let Some(set) = provider.emote_sets.get_mut(&set_id) && let Some(emote) = set.get_mut(&name) {
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
          if providers.contains_key(&ProviderName::Twitch) == false {
            providers.insert(ProviderName::Twitch, Provider {
                name: "twitch".to_owned(),
                emote_sets: Default::default(),
                emotes: Default::default(),
                global_badges: emote_loader.twitch_get_global_badges(&auth_tokens.twitch_auth_token)
            });
          }
          twitch::init_channel(&auth_tokens.twitch_username, &auth_tokens.twitch_auth_token, channel_options.channel_name.to_owned(), self.runtime.as_ref().unwrap(), emote_loader)
        },
        ProviderName::YouTube => {
          if providers.contains_key(&ProviderName::Twitch) == false {
            providers.insert(ProviderName::Twitch, Provider {
                name: "youtube".to_owned(),
                emote_sets: Default::default(),
                emotes: Default::default(),
                global_badges: Default::default()
            });
          }
          youtube::init_channel(channel_options.channel_name.to_owned(), channel_options.channel_id.to_owned(), auth_tokens.youtube_auth_token.to_owned(), self.runtime.as_ref().unwrap())
        },
        _ => panic!("invalid provider")
      };

      self.channels.insert(channel_options.channel_name.to_owned(), c);
      *(&mut self.selected_channel) = Some(channel_options.channel_name.to_owned());
      channel_options.channel_name = Default::default();
    };

    if self.show_auth_ui {
      egui::Window::new("Auth Tokens").show(ctx, |ui| {
        ui.label("Twitch");
        ui.horizontal(|ui| {
          ui.label("Username:");
          ui.text_edit_singleline(&mut self.auth_tokens.twitch_username);
        });
        ui.horizontal(|ui| {
          ui.label("Token:");
          ui.text_edit_singleline(&mut self.auth_tokens.twitch_auth_token);
          if ui.button("Log In").clicked() {
            twitch::authenticate(self.runtime.as_ref().unwrap());
          }
        });
        /*ui.horizontal(|ui| {
          ui.label("YouTube");
          ui.text_edit_singleline(&mut self.auth_tokens.youtube_auth_token);
        });*/
        if ui.button("Ok").clicked() {
          let twitch_token = self.auth_tokens.twitch_auth_token.to_owned();
          if twitch_token.starts_with("#") || twitch_token.starts_with("access") {
            let rgx = regex::Regex::new("access_token=(.*?)&").unwrap();
            let cleaned = rgx.captures(twitch_token.as_str()).unwrap().get(1).map_or("", |x| x.as_str());
            self.auth_tokens.twitch_auth_token = cleaned.to_owned();
          }

          self.show_auth_ui = false;
        }
      });
    }

    if self.add_channel_menu_show {
      egui::Window::new("Add Channel").show(ctx, |ui| {
        let mut name_input : Option<egui::Response> = None;
        ui.horizontal(|ui| {
          ui.label("Provider:");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::Twitch, "Twitch");
          //ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::YouTube, "Youtube");
          //ui.selectable_value(&mut self.add_channel_menu_provider, ProviderName::Null, "destiny.gg");
          //ui.selectable_value(&mut self.add_channel_menu_provider, ProviderName::Null, "Null");
        });
        ui.horizontal(|ui| {
          ui.label("Channel Name:");
          name_input = Some(ui.text_edit_singleline(&mut self.add_channel_menu.channel_name));
          //name_input.request_focus();
          
        });
        if self.add_channel_menu.provider == ProviderName::YouTube {
          ui.horizontal(|ui| {
            ui.label("Channel ID:");
            ui.text_edit_singleline(&mut self.add_channel_menu.channel_id);
          });
        }
        
        if name_input.unwrap().has_focus() && ui.input().key_pressed(egui::Key::Enter) || ui.button("Add channel").clicked() {
          add_channel(&mut self.providers, &mut self.auth_tokens, &mut self.add_channel_menu, self.emote_loader.as_mut().unwrap()); 
          self.add_channel_menu_show = false;
        }
      });
    }

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      ui.horizontal(|ui| {
        egui::menu::bar(ui, |ui| {
          ui.menu_button("File", |ui| {
            ui.set_width(ui.available_width());
            if ui.button("Configure Tokens").clicked() {
              *(&mut self.show_auth_ui) = true;
              ui.close_menu();
            }
            if ui.button("Add a channel").clicked() {
              *(&mut self.add_channel_menu_show) = true;
              ui.close_menu();
            }
            if ui.button("Quit").clicked() {
              frame.quit();
            }
          });
        });
        egui::warn_if_debug_build(ui);
      });
      ui.separator();

      let mut channel_removed = false;
      ui.horizontal(|ui| {
        let label = RichText::new("All Channels").size(BUTTON_TEXT_SIZE);
        let clbl = ui.selectable_value(&mut self.selected_channel, None, label);
        if clbl.clicked() {
          channel_swap = true;
        }

        let channels = &mut self.channels;
        for (channel, sco) in channels.iter_mut() {  
          if let Some(t) = sco.transient.as_mut() {
            
            let emote_loader = self.emote_loader.as_mut().unwrap();
            let providers = &mut self.providers;

            while let Ok(x) = t.rx.try_recv() {
              match x {
                InternalMessage::PrivMsg { message } => {
                  //sco.history.insert(sco.history.len(), message)
                  self.chat_history.push_back((message, None));
                },
                InternalMessage::StreamingStatus { is_live } => {
                  t.is_live = is_live;
                },
                InternalMessage::MsgEmotes { emote_ids } => {
                  if let Some(provider) = providers.get_mut(&sco.provider) {
                    for (id, name) in emote_ids {
                      if provider.emotes.contains_key(&name) == false {
                        provider.emotes.insert(name.to_owned(), emotes::fetch::get_emote(name, id, "".to_owned(), "generated/twitch/".to_owned(), None));
                      }
                    }
                  }
                },
                InternalMessage::RoomId { room_id } => {
                  sco.roomid = room_id;
                  match emote_loader.load_channel_emotes(&sco.roomid) {
                    Ok(x) => {
                      t.channel_emotes = Some(x);
                    },
                    Err(x) => { 
                      println!("ERROR LOADING CHANNEL EMOTES: {}", x); 
                      Default::default()
                    }
                  };
                  t.badge_emotes = emote_loader.twitch_get_channel_badges(&self.auth_tokens.twitch_auth_token, &sco.roomid);
                  break;
                },
                InternalMessage::EmoteSets { emote_sets } => {
                  if let Some(provider) = providers.get_mut(&sco.provider) {
                    for set in emote_sets {
                      if provider.emote_sets.contains_key(&set) == false && let Some(set_list) = emote_loader.twitch_get_emote_set(&self.auth_tokens.twitch_auth_token, &set) {
                        provider.emote_sets.insert(set.to_owned(), set_list);
                      }
                    }
                  }
                }
              };
            }

            let label = RichText::new(format!("{} {}", channel, match t.is_live { true => "🔴", false => ""}))
            .size(BUTTON_TEXT_SIZE);
            let clbl = ui.selectable_value(&mut self.selected_channel, Some(channel.to_owned()), label);
            if clbl.clicked() {
              channel_swap = true;
            }
            else if clbl.middle_clicked() { //TODO: custom widget that adds close button?
              //self.runtime.block_on(async {
                _ = sco.close();
              //});
              channel_removed = true;
            }
          }
          else {
            // channel has not been opened yet
            twitch::open_channel(&self.auth_tokens.twitch_username, &self.auth_tokens.twitch_auth_token, sco, self.runtime.as_ref().unwrap(), self.emote_loader.as_ref().unwrap());
          }
        }
      });
      if channel_removed {
        if let Some(name) = &self.selected_channel {
          self.channels.remove(name);
        }
        *(&mut self.selected_channel) = None;
      }
    });

    let cframe = egui::Frame { 
      inner_margin: egui::style::Margin::same(5.0), 
      fill: egui::Color32::from(egui::Color32::TRANSPARENT),
      ..Default::default() 
    };
    
    egui::CentralPanel::default().frame(cframe).show(ctx, |ui| {
      ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
        if let Some(sc) = &self.selected_channel.to_owned() {
          let goto_next_emote = self.selected_emote.is_some() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowRight);
          let goto_prev_emote = self.selected_emote.is_some() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowLeft);
          let enter_emote = self.selected_emote.is_some() && ui.input_mut().consume_key(Modifiers::ALT, Key::ArrowDown);

          let mut outgoing_msg = egui::TextEdit::multiline(&mut self.draft_message)
            .desired_rows(2)
            .desired_width(ui.available_width())
            .hint_text("Type a message to send")
            .font(egui::TextStyle::Body)
            .show(ui);
          if outgoing_msg.response.has_focus() && ui.input().key_down(egui::Key::Enter) && ui.input().modifiers.shift == false && self.draft_message.len() > 0 {
            if let Some(sco) = (&mut self.channels).get_mut(sc) && let Some(t) = sco.transient.as_mut() {
              match t.tx.try_send(OutgoingMessage::Chat { message: (&mut self.draft_message).replace("\n", " ").to_owned() }) {
                Err(e) => println!("Failed to send message: {}", e), //TODO: emit this into UI
                _ => ()
              } 
              *(&mut self.draft_message) = String::new();
            }
          }
          else if self.draft_message.len() > 0 && let Some(cursor_pos) = outgoing_msg.state.ccursor_range() {
            let cursor = cursor_pos.primary.index;
            let emotes = self.get_possible_emotes(cursor);
            if let Some((word, pos, emotes)) = emotes && emotes.len() > 0 {
              if enter_emote && let Some(emote_text) = &self.selected_emote {
                let msg = if self.draft_message.len() <= pos + &word.len() || &self.draft_message[pos + &word.len()..pos + &word.len() + 1] != " " {
                  format!("{}{} {}",&self.draft_message[..pos], emote_text, &self.draft_message[pos + &word.len()..])
                } else {
                  format!("{}{}{}",&self.draft_message[..pos], emote_text, &self.draft_message[pos + &word.len()..])
                };
                self.draft_message = msg;
                outgoing_msg.response.request_focus();
                outgoing_msg.state.set_ccursor_range(
                  Some(egui::text_edit::CCursorRange::one(egui::text::CCursor::new(&self.draft_message[..pos].len() + emote_text.len() + 1)))
                );
                outgoing_msg.state.store(ctx, outgoing_msg.response.id);
                self.selected_emote = None;
              }
              else {
                if goto_next_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == self.selected_emote.as_ref()) && ix + 1 < emotes.len() {
                  self.selected_emote = emotes.get(ix + 1).and_then(|x| Some(x.0.to_owned()));
                }
                else if goto_prev_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == self.selected_emote.as_ref()) && ix > 0 {
                  self.selected_emote = emotes.get(ix - 1).and_then(|x| Some(x.0.to_owned()));
                }
                else if self.selected_emote.is_none() || emotes.iter().any(|x| Some(&x.0) == self.selected_emote.as_ref()) == false {
                  self.selected_emote = emotes.first().and_then(|x| Some(x.0.to_owned()));
                }

                ui.allocate_ui_with_layout(emath::Vec2::new(ui.available_width(), 35. + EMOTE_HEIGHT * 2.), 
                    egui::Layout::from_main_dir_and_cross_align( egui::Direction::LeftToRight, Align::BOTTOM), |ui| {
                egui::ScrollArea::horizontal()
                .id_source("emote_selector_scrollarea")
                .always_show_scroll(true)
                .show(ui, |ui|{
                    for emote in emotes {
                      if goto_prev_emote && self.selected_emote.as_ref() == Some(&emote.0) {
                        ui.scroll_to_cursor(None)
                      }
                      ui.vertical(|ui| {
                        if let Some(img) = emote.1 {
                          if ui.image(&img.texture, egui::vec2(&img.texture.size_vec2().x * (EMOTE_HEIGHT * 2. / &img.texture.size_vec2().y), EMOTE_HEIGHT * 2.))
                              .interact(egui::Sense::click())
                              .clicked() {
                            self.selected_emote = Some(emote.0.to_owned());
                          }
                        }
                        else {
                          ui.add_space(EMOTE_HEIGHT);
                        }

                        ui.style_mut().wrap = Some(false);
                        let mut disp_text = emote.0.to_owned();
                        if disp_text.len() > 12 {
                          disp_text.truncate(10);
                          disp_text.push_str("...");
                        }
                        ui.selectable_value(&mut self.selected_emote, Some(emote.0.to_owned()), RichText::new(disp_text).size(SMALL_TEXT_SIZE))
                          .on_hover_text_at_pointer(emote.0.to_owned());
                      });
                      if goto_next_emote && self.selected_emote.as_ref() == Some(&emote.0) {
                        ui.scroll_to_cursor(None)
                      }
                    }
                  });
                });
                ui.label("Alt+Left/Right to select, Alt+Down to confirm");
              }
            }
          }
          ui.separator();
        }
        
        let mut popped_height = 0.;
        while self.chat_history.len() > 2000 {
          if let Some(popped) = self.chat_history.pop_front() && let Some(height) = popped.1 {
            if self.selected_channel.is_none() || self.selected_channel == Some(popped.0.channel) {
              popped_height += height + ui.spacing().item_spacing.y;
            }
          }
        }

        let chat_area = egui::ScrollArea::vertical()
          .auto_shrink([false; 2])
          .stick_to_bottom()
          .always_show_scroll(true)
          .scroll_offset(self.chat_scroll.and_then(|f| Some(egui::Vec2 {x: 0., y: f.y - popped_height }) ).or_else(|| Some(egui::Vec2 {x: 0., y: 0.})).unwrap());
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

  fn on_exit_event(&mut self) -> bool {
    true
  }

  fn on_exit(&mut self, _ctx : &eframe::glow::Context) {
    //self.emote_loader.tx.try_send(EmoteRequest::Shutdown);
    self.emote_loader.as_ref().unwrap().close();
    for channel in self.channels.values_mut() {
      //self.runtime.block_on(async move {
        _ = channel.close();//.await;
      //});
    }
  }

  fn auto_save_interval(&self) -> std::time::Duration {
      std::time::Duration::from_secs(30)
  }

  fn max_size_points(&self) -> egui::Vec2 {
    egui::Vec2::new(1024.0, 2048.0)
  }

  fn clear_color(&self) -> egui::Rgba {
    egui::Color32::from_rgba_premultiplied(0, 0, 0, 200).into()
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
  fn show_variable_height_rows(&mut self, ui : &mut egui::Ui, viewport: emath::Rect, channel_name: &Option<String>) {
    ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
      ui.spacing_mut().item_spacing.x = 4.0;
      //ui.spacing_mut().item_spacing.y = 1.;

      let y_min = ui.max_rect().top() + viewport.min.y;
      let y_max = ui.max_rect().top() + viewport.max.y;
      let rect = emath::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
  
      let mut in_view : Vec<(&ChatMessage, HashMap<String, EmoteFrame>, Option<HashMap<String, EmoteFrame>>, Vec<(f32, Option<usize>)>, Vec<bool>, f32)> = Vec::default();
      let mut y_pos = 0.0;
      let mut excess_top_space : Option<f32> = None;
      let mut skipped_rows = 0;

      for (row, cached_y) in self.chat_history.iter_mut() {
        if let Some(channel) = channel_name && &row.channel != channel {
          continue;
        }
        // Skip processing if row size is accurately cached and not in view
        else if let Some(last_viewport) = self.chat_frame && last_viewport.size() == viewport.size() && let Some(size_y) = cached_y.as_ref()
          && (y_pos < viewport.min.y - 200. || y_pos + size_y > viewport.max.y + excess_top_space.unwrap_or(0.) + 200.) {
            y_pos += size_y + ui.spacing().item_spacing.y;
            if y_pos < viewport.min.y - 200. {
              skipped_rows += 1;
            }
            continue;
        }

        let (provider_emotes, provider_badges) = self.providers.get_mut(&ProviderName::Twitch)
          .and_then(|p| Some((Some(&mut p.emotes), p.global_badges.as_mut()))).unwrap_or((None, None));
        let (channel_emotes, channel_badges) = self.channels.get_mut(&row.channel)
          .and_then(|c| c.transient.as_mut())
          .and_then(|t| Some((t.channel_emotes.as_mut(), t.badge_emotes.as_mut()))).unwrap_or((None, None));
        let emotes = get_emotes_for_message(&row, &row.channel, provider_emotes, channel_emotes, &mut self.global_emotes, self.emote_loader.as_mut().unwrap());
        let badges = get_badges_for_message(row.profile.badges.as_ref(), &row.channel, provider_badges, channel_badges, self.emote_loader.as_mut().unwrap());
        let msg_sizing = chat_estimate::get_chat_msg_size(ui, &row, &emotes, badges.as_ref());
        *cached_y = Some(msg_sizing.iter().map(|x| x.0).sum::<f32>());
        
        let mut lines_to_include : Vec<bool> = Default::default();
        let mut row_y = 0.;
        for line in &msg_sizing {
          let size_y = line.0;
          if y_pos + row_y >= viewport.min.y && y_pos + row_y + size_y <= viewport.max.y + excess_top_space.unwrap_or(0.) {
            if excess_top_space.is_none() {
              excess_top_space = Some(y_pos + row_y - viewport.min.y);
            }

            lines_to_include.insert(lines_to_include.len(), true);
          } 
          else {
            lines_to_include.insert(lines_to_include.len(), false);
          }
          row_y += size_y + ui.spacing().item_spacing.y;
        }
        y_pos += row_y;
        if (&lines_to_include).iter().any(|x| *x) {
          in_view.push((row, emotes, badges, msg_sizing, lines_to_include, row_y));
        }
      }
      self.chat_frame = Some(viewport.to_owned());
      ui.set_height(y_pos);
      ui.skip_ahead_auto_ids(skipped_rows);
      ui.allocate_ui_at_rect(rect, |viewport_ui| {
        for (row, emotes, badges, sizing, row_include, _row_expected_y) in in_view {
          let _actual = chat::create_chat_message(viewport_ui, row, &emotes, badges.as_ref(), self.emote_loader.as_mut().unwrap(), sizing, row_include);
          //println!("expected {} actual {} for {}", _row_expected_y, _actual.size().y, &row.username);
        }
      });
    });
  }

  fn get_possible_emotes(&mut self, cursor_position: usize) -> Option<(String, usize, Vec<(String, Option<EmoteFrame>)>)> {
    let msg = &self.draft_message;
    let word : Option<(usize, &str)> = msg.split_whitespace()
      .map(move |s| (s.as_ptr() as usize - msg.as_ptr() as usize, s))
      .filter_map(|p| if p.0 <= cursor_position && cursor_position <= p.0 + p.1.len() { Some((p.0, p.1)) } else { None })
      .next();

    if let Some((pos, word)) = word {
      if word.len() < 3 {
        return None;
      }
      let word_lower = &word.to_lowercase();

      let mut starts_with_emotes : HashMap<String, Option<EmoteFrame>> = Default::default();
      let mut contains_emotes : HashMap<String, Option<EmoteFrame>> = Default::default();
      // Find similar emotes. Show emotes starting with same string first, then any that contain the string.
      if let Some(channel_name) = &self.selected_channel && let Some(channel) = self.channels.get_mut(channel_name) && let Some(transient) = channel.transient.as_mut() && let Some(channel_emotes) = transient.channel_emotes.as_mut() {
        for (name, emote) in channel_emotes { // Channel emotes
          if name == word {
            return None;
          }
          let name_l = name.to_lowercase();
          if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
            let tex = chat::get_texture(self.emote_loader.as_mut().unwrap(), emote, EmoteRequest::new_channel_request(&emote, &channel_name));
            _ = match name_l.starts_with(word_lower) {
              true => starts_with_emotes.try_insert(name.to_owned(), tex),
              false => contains_emotes.try_insert(name.to_owned(), tex),
            };
          }
        }
        if let Some(provider) = self.providers.get_mut(&channel.provider) { // Provider emotes
          for (set_id, emote_set) in &mut provider.emote_sets {
            for (name, emote) in emote_set {
              if name == word {
                return None;
              }
              let name_l = name.to_lowercase();
              if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
                let tex = chat::get_texture(self.emote_loader.as_mut().unwrap(), emote, EmoteRequest::new_emoteset_request(&emote, &channel.provider, set_id));
                _ = match name_l.starts_with(word_lower) {
                  true => starts_with_emotes.try_insert(name.to_owned(), tex),
                  false => contains_emotes.try_insert(name.to_owned(), tex),
                };
              }
            }
          }
        }
      }
      for (name, emote) in &mut self.global_emotes { // Global emotes
        if name == word {
          return None;
        }
        let name_l = name.to_lowercase();
        if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
          let tex = chat::get_texture(self.emote_loader.as_mut().unwrap(), emote, EmoteRequest::new_global_request(&emote));
          _ = match name_l.starts_with(word_lower) {
            true => starts_with_emotes.try_insert(name.to_owned(), tex),
            false => contains_emotes.try_insert(name.to_owned(), tex),
          };
        }
      }
      let mut starts_with = starts_with_emotes.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).collect_vec();
      let mut contains = contains_emotes.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).collect_vec();
      starts_with.append(&mut contains);
      Some((word.to_owned(), pos, starts_with))
    }
    else {
      None
    }
  }
}

fn get_emotes_for_message(row: &ChatMessage, channel_name: &str, provider_emotes: Option<&mut HashMap<String, Emote>>, channel_emotes: Option<&mut HashMap<String, Emote>>, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> HashMap<String, EmoteFrame> {
  let mut result : HashMap<String, chat::EmoteFrame> = Default::default();
  for word in row.message.to_owned().split(" ") {
    let emote = 
      if let Some(&mut ref mut channel_emotes) = channel_emotes && let Some(emote) = channel_emotes.get_mut(word) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_request(emote, channel_name))
      }
      else if let Some(emote) = global_emotes.get_mut(word) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_global_request(emote))
      }
      else if let Some(&mut ref mut provider_emotes) = provider_emotes && let Some(emote) = provider_emotes.get_mut(word) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_twitch_msg_emote_request(emote))
      }
      else {
        None
      };
    if let Some(frame) = emote {
      result.insert(frame.name.to_owned(), frame);
    }
  }

  result
}

fn get_badges_for_message(badges: Option<&Vec<String>>, channel_name: &str, global_badges: Option<&mut HashMap<String, Emote>>, channel_badges: Option<&mut HashMap<String, Emote>>, emote_loader: &mut EmoteLoader) -> Option<HashMap<String, EmoteFrame>> {
  let mut result : HashMap<String, chat::EmoteFrame> = Default::default();
  if badges.is_none() { return None; }
  for badge in badges.unwrap() {
    let emote = 
      if let Some(&mut ref mut channel_badges) = channel_badges && let Some(emote) = channel_badges.get_mut(badge) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_badge_request(emote, channel_name))
      }
      else if let Some(&mut ref mut global_badges) = global_badges && let Some(emote) = global_badges.get_mut(badge) {
        chat::get_texture(emote_loader, emote, EmoteRequest::new_global_badge_request(emote))
      }
      else {
        None
      };
    if let Some(frame) = emote {
      result.insert(frame.name.to_owned(), frame);
    }
  }

  Some(result)
}