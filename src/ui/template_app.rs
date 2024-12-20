/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use tracing::{info, error, warn, debug};
use tracing_unwrap::{OptionExt, ResultExt};
use std::{collections::HashMap, ops::Add};
use chrono::{DateTime, Utc};
use egui::{emath::{Align, Rect}, text_edit::TextEditState, Context, Key, Modifiers, OpenUrl, Pos2, Response, RichText, Rounding, Stroke, TextStyle, TextureHandle};
use egui::{Vec2, text::LayoutJob, Color32};
use image::DynamicImage;
use itertools::Itertools;
use crate::{provider::{dgg, twitch::{self, TwitchChatManager}, youtube_server, ChatManagerRx, ChatMessage, IncomingMessage, MessageType, OutgoingMessage, Provider, ProviderName}, ui::addtl_functions::update_font_sizes};
use crate::provider::channel::{Channel, ChannelTransient, ChannelUser, YoutubeChannel, ChannelShared};
use crate::emotes::{LoadEmote, AddEmote, OverlayItem, EmoteSource};
use crate::{emotes, emotes::{Emote, EmoteLoader, EmoteRequest, EmoteResponse, imaging::load_image_into_texture_handle}};

use super::{addtl_functions::*, chat, consts::*, ChatPanelOptions, SelectorFormat, TemplateApp, UiEvent};

use super::models::*;

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Runtime) -> Self {
      cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
      let mut r = TemplateApp {
        body_text_size: 14.0,
        chat_history_limit: 2000,
        ..Default::default()
      };
  
      update_font_sizes(&r, &cc.egui_ctx);
  
      #[cfg(feature = "persistence")]
      if let Some(storage) = cc.storage && let Some(stored) = eframe::get_value(storage, eframe::APP_KEY) {
          r = stored
      } else {
          r = TemplateApp { ..Default::default() };
          r.chat_history_limit = 100;
      }
      r.emote_loader = EmoteLoader::new("Gigachat", &runtime);
      r.emote_loader.transparent_img = Some(load_image_into_texture_handle(&cc.egui_ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 0]) )))));
      r.runtime = Some(runtime);
      info!("{} channels", r.channels.len());
  
      if r.twitch_chat_manager.is_none() && !r.auth_tokens.twitch_username.is_empty() && !r.auth_tokens.twitch_auth_token.is_empty() {
        r.twitch_chat_manager = Some(TwitchChatManager::new(&r.auth_tokens.twitch_username, &r.auth_tokens.twitch_auth_token, r.runtime.as_ref().unwrap_or_log(), &cc.egui_ctx));
  
        match r.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { token: r.auth_tokens.twitch_auth_token.to_owned(), force_redownload: false }) {  
          Ok(_) => {},
          Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
        };
      }
      r
    }

  pub fn update_inner(&mut self, ctx: &egui::Context) {
    if self.emote_loader.transparent_img.is_none() {
      self.emote_loader.transparent_img = Some(load_image_into_texture_handle(ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([100, 100, 100, 0]) )))));
    }

    if self.emote_loader.red_img.is_none() {
        self.emote_loader.red_img = Some(load_image_into_texture_handle(ctx, emotes::imaging::to_egui_image(DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([254, 100, 100, 254]) )))));
    }

    if self.yt_chat_manager.is_none() && self.enable_yt_integration {
      self.yt_chat_manager = Some(youtube_server::start_listening(self.runtime.as_ref().unwrap()));
    }

    // workaround for odd rounding issues at certain DPI(s?)
    /*if ctx.pixels_per_point() == 1.75 {
      ctx.set_pixels_per_point(1.50);
    }*/

    //let mut i = 0;
    while let Ok(event) = self.emote_loader.rx.try_recv() {
      let loading_emotes = &mut self.emote_loader.loading_emotes;
      match event {
        EmoteResponse::GlobalEmoteListResponse { response } => {
          match response {
            Ok(x) => {
              for (name, mut emote) in x {
                emote.source = EmoteSource::Global;
                self.global_emotes.insert(name, emote);
              }
            },
            Err(x) => { error!("Error loading global emotes: {}", x); }
          };
        },
        EmoteResponse::GlobalEmoteImageLoaded { name, data } => { self.update_emote(&name, ctx, data); },
        EmoteResponse::TwitchGlobalBadgeListResponse { response } => {
          match response {
            Ok(mut badges) => {
              for (_, badge) in badges.iter_mut() {
                badge.source = EmoteSource::GlobalBadge;
              }
              if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) {
                provider.global_badges = Some(badges)
              }
            },
            Err(e) => { error!("Failed to load twitch global badge json due to error {:?}", e); }
          }
        },
        EmoteResponse::GlobalBadgeImageLoaded { name, data } => {
          if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) {
            provider.update_badge(&name, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) {
            match channel {
              Channel::DGG { dgg: _, shared } |
              Channel::Twitch { twitch: _, shared } | 
              Channel::Youtube { youtube: _, shared } => shared.update_emote(&name, ctx, data, loading_emotes),
            }
          }
        },
        EmoteResponse::ChannelBadgeImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) {
            match channel {
              Channel::DGG { dgg: _, shared } | 
              Channel::Twitch { twitch: _, shared } | 
              Channel::Youtube { youtube: _, shared } => shared.update_badge(&name, ctx, data, loading_emotes),
            }
          }
        },
        EmoteResponse::TwitchMsgEmoteLoaded { name, id: _, data } => {
          if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) {
            provider.update_emote(&name, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::YouTubeMsgEmoteLoaded { name, data } => {
          if let Some(provider) = self.providers.get_mut(&ProviderName::YouTube) {
            provider.update_emote(&name, ctx, data, loading_emotes);
          }
        },
        EmoteResponse::TwitchEmoteSetResponse { emote_set_id: _, response } => {
          if let Ok(set_list) = response && let Some(provider) = self.providers.get_mut(&ProviderName::Twitch)  {
            for (_id, mut emote) in set_list {
              emote.source = EmoteSource::Twitch;
              provider.my_sub_emotes.insert(emote.name.to_owned());
              if !provider.emotes.contains_key(&emote.name) {
                provider.emotes.insert(emote.name.to_owned(), emote);
              }
            }
          }
        },
        EmoteResponse::ChannelEmoteListResponse { channel_name, response } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) {
            channel.set_emotes(response);
          }
        },
        EmoteResponse::ChannelBadgeListResponse { channel_name, response } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) {
            channel.set_badges(response);
          }
        }
      }
      //i += 1;
      //if i > 5 { break; }
      ctx.request_repaint();
    }

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
    let mut msglist : Vec<IncomingMessage> = Vec::new();
    for (_, channel) in self.channels.iter_mut() {
      if let Channel::DGG { dgg, shared: _ } = channel {
        while let Some(chat_mgr) = dgg.dgg_chat_manager.as_mut() && let Ok(x) = chat_mgr.out_rx.try_recv() {
          msglist.push(x);
          msgs += 1;
          if msgs > 20 { break; } // Limit to prevent bad UI lag
        }
        msgs = 0;
      }
    }
    for x in msglist {
        self.handle_incoming_message(x);
    }
    
    while let Some(chat_mgr) = self.yt_chat_manager.as_mut()  && let Ok(x) = chat_mgr.out_rx.try_recv() {
      self.handle_incoming_message(x);
      msgs += 1;
      if msgs > 20 { break; } // Limit to prevent bad UI lag
    }

    let body_font_size = self.body_text_size;

    let tframe = egui::Frame { 
        inner_margin: egui::epaint::Margin::same(3.), 
        outer_margin: egui::epaint::Margin::same(0.),
        fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, self.bg_transparency),
        ..Default::default() 
      };

    egui::TopBottomPanel::top("top_panel")
    .frame(tframe)
    .show(ctx, |ui| {

      self.render_menubar(ui, ctx);
      
      ui.separator();

      if let Some(result) = self.render_channel_tab_component(ui, ctx) && let UiEvent::ChannelRemoved(removed_channel) = result {
        channel_removed = Some(removed_channel);
      }
    });

    if body_font_size != self.body_text_size {
      update_font_sizes(self, ctx);
    }

    let lhs_chat_state = ChatPanelOptions {
        selected_channel: self.selected_channel.to_owned(),
        draft_message: self.lhs_chat_state.draft_message.to_owned(),
        chat_frame: self.lhs_chat_state.chat_frame.to_owned(),
        chat_scroll: self.lhs_chat_state.chat_scroll.to_owned(),
        selected_user: self.lhs_chat_state.selected_user.to_owned(),
        selected_msg: self.lhs_chat_state.selected_msg.to_owned(),
        selected_emote: self.lhs_chat_state.selected_emote.to_owned(),
        selected_emote_input: self.lhs_chat_state.selected_emote_input.to_owned()
    };

    let mut popped_height = 0.;
    let mut rhs_popped_height = 0.;
    for (_channel, history) in self.chat_histories.iter_mut() {
      if history.len() > self.chat_history_limit && let Some(popped) = history.pop_front() 
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
      inner_margin: egui::epaint::Margin::same(3.), 
      outer_margin: egui::epaint::Margin::same(0.),
      fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, self.bg_transparency),
      ..Default::default() 
    };

    egui::CentralPanel::default()
    .frame(cframe)
    .show(ctx, |ui| {
      let height = ui.available_height();
      ui.horizontal(|ui| {
        ui.set_height(height);
        
        let rhs_active = self.rhs_selected_channel.is_some();
        let lhs_response = self.show_chat_frame("lhs", ui, lhs_chat_state, ctx, rhs_active, popped_height);
        self.lhs_chat_state = lhs_response.state;

        let rhs_response = if self.rhs_selected_channel.is_some() {
            let rhs_chat_state = ChatPanelOptions {
                selected_channel: self.rhs_selected_channel.to_owned(),
                draft_message: self.rhs_chat_state.draft_message.to_owned(),
                chat_frame: self.rhs_chat_state.chat_frame.to_owned(),
                chat_scroll: self.rhs_chat_state.chat_scroll.to_owned(),
                selected_user: self.rhs_chat_state.selected_user.to_owned(),
                selected_msg: self.rhs_chat_state.selected_msg.to_owned(),
                selected_emote: self.rhs_chat_state.selected_emote.to_owned(),
                selected_emote_input: self.rhs_chat_state.selected_emote_input.to_owned()
            };
            self.rhs_chat_state.selected_channel = self.rhs_selected_channel.to_owned();

            //let mut rhs_chat_state = self.rhs_chat_state.to_owned();
            let rhs_response = self.show_chat_frame("rhs", ui, rhs_chat_state, ctx, false, rhs_popped_height);
            self.rhs_chat_state = rhs_response.state;
            Some(ChatFrameResponse { y_size: rhs_response.y_size, ..Default::default() })
        } else { None };

        for _ in 0..self.last_frame_ui_events.len() {
          match self.last_frame_ui_events.pop_front() {
            Some(UiEvent::ChannelChangeLHS) => { 
                self.lhs_chat_state.chat_scroll = Some(Vec2 { x: 0., y: lhs_response.y_size }); 
            },
            Some(UiEvent::ChannelChangeRHS) => { 
                self.rhs_chat_state.chat_scroll = Some(Vec2 { x: 0., y: rhs_response.as_ref().map(|f| f.y_size).unwrap_or(0.) }); 
            },
            Some(event) => self.last_frame_ui_events.push_back(event),
            _ => warn!("unexpected failure to pop last_frame_ui_events")
          }
        }

        let rhs_rect = ui.max_rect().shrink2(Vec2::new(ui.max_rect().width() * 0.25, 0.)).translate(Vec2::new(ui.max_rect().width() * 0.25, 0.));

        let drag_state_needs_reset = match &self.dragged_channel_tab {
            DragChannelTabState::DragStart(_channel, drag_start_tab_list) => {
                if let Some(pos) = ctx.pointer_latest_pos() && rhs_rect.contains(pos) {
                    // revert any change to tab order while dragging
                    self.channel_tab_list = drag_start_tab_list.to_owned();

                    //paint rectangle to indicate drop will shift to other chat panel
                    ui.painter().rect_filled(rhs_rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(40,40,40,150));
                }
                false
            },
            DragChannelTabState::DragRelease(drag_channel, tab_order_changed, pos) => {
                if rhs_rect.contains(*pos) {
                    if self.selected_channel.as_ref() == Some(drag_channel) {
                        self.selected_channel = if self.rhs_selected_channel.is_some() {
                            self.rhs_selected_channel.to_owned()
                        } else { None };
                    }
                    self.rhs_selected_channel = Some(drag_channel.to_owned());
                    self.last_frame_ui_events.push_back(UiEvent::ChannelChangeRHS);
                    true
                } else if !tab_order_changed {
                    self.selected_channel = Some(drag_channel.to_owned());
                    self.last_frame_ui_events.push_back(UiEvent::ChannelChangeLHS);
                    true
                } else {
                    false
                }
            },
            DragChannelTabState::None => false
        };

        if drag_state_needs_reset {
            self.dragged_channel_tab = DragChannelTabState::None;
        }
        
      });
    });

    

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

    if let Some(channel) = channel_removed {
      if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
        chat_mgr.leave_channel(&channel);
      }
      self.channels.remove(&channel);
      self.channel_tab_list = self.channel_tab_list.iter().filter_map(|f| if f != &channel { Some(f.to_owned()) } else { None }).collect_vec();
    }

    //ctx.request_repaint();
  }

  fn handle_incoming_message(&mut self, x: IncomingMessage) {
    match x {
      IncomingMessage::PrivMsg { mut message } => {
        let provider_emotes = self.providers.get(&message.provider).map(|f| &f.emotes);
        let channel = message.channel.to_owned();
        // remove any extra whitespace between words
        let rgx = regex::Regex::new("\\s+").unwrap_or_log();
        message.message = rgx.replace_all(message.message.trim_matches(' '), " ").to_string();

        // strip down links in YT chats
        if message.provider == ProviderName::YouTube && message.message.contains("https://www.youtube.com/redirect") {
          let rgx = regex::Regex::new("http[^\\s]*q=([^\\s]*)").unwrap_or_log();
          let str = rgx.replace_all(&message.message, "$1");
          message.message = urlencoding::decode(&str).map(|x| x.into_owned()).unwrap_or_else(|_| str.to_string());
        }

        if message.provider == ProviderName::YouTube && !self.channels.contains_key(&message.channel) {
          self.channel_tab_list.push(message.channel.to_owned());
          self.channels.insert(message.channel.to_owned(), Channel::Youtube { 
            youtube: YoutubeChannel {}, 
            shared: ChannelShared { 
              channel_name: message.channel.to_owned(),  
              transient: Some(ChannelTransient { 
                channel_emotes: None,
                badge_emotes: None,
                status: None }),
              ..Default::default() 
            }    
          });
        }

        if message.username.is_empty() && message.channel.is_empty() && message.msg_type != MessageType::Chat {
          let provider_channels = self.channels.iter().filter_map(|(_, c)| {
            if c.provider() == message.provider { 
              Some(c.channel_name().to_owned())
            } else {
              None
            }
          }).collect_vec();
          for channel in provider_channels {
            let chat_history = self.chat_histories.entry(channel.to_owned()).or_default();
            push_history(
              chat_history, 
              message.to_owned(),
              provider_emotes, 
              self.channels.get(&channel).and_then(|f| f.transient()).and_then(|f| f.channel_emotes.as_ref()),
              &self.global_emotes);
          }
        } else {
          let chat_history = self.chat_histories.entry(channel.to_owned()).or_default();

          if let Some(c) = self.channels.get_mut(&channel) {
            c.shared_mut().users.insert(message.username.to_lowercase(), ChannelUser {
              username: message.username.to_owned(),
              display_name: message.profile.display_name.as_ref().unwrap_or(&message.username).to_owned(),
              is_active: true
            });
            // Twitch has some usernames that have completely different display names (e.g. Japanese character display names)
            if let Some(display_name) = message.profile.display_name.as_ref() && display_name.to_lowercase() != message.username.to_lowercase() {
              c.shared_mut().users.insert(display_name.to_lowercase(), ChannelUser {
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
            self.channels.get(&channel).and_then(|f| f.transient()).and_then(|f| f.channel_emotes.as_ref()),
            &self.global_emotes);
        }
      },
      IncomingMessage::StreamingStatus { channel, status } => {
        if let Some(t) = self.channels.get_mut(&channel).and_then(|f| f.transient_mut()) {
          t.status = status;
        }
      },
      IncomingMessage::MsgEmotes { provider, emote_ids } => {
        if let Some(p) = self.providers.get_mut(&provider) {
          for (id, name) in emote_ids {
            match provider {
              ProviderName::Twitch => if !p.emotes.contains_key(&name) {
                p.emotes.insert(name.to_owned(), Emote { name, id, url: "".to_owned(), path: "twitch/".to_owned(), source: EmoteSource::Twitch, ..Default::default() });
              },
              ProviderName::YouTube => if !p.emotes.contains_key(&name) {
                p.emotes.insert(id.to_owned(), Emote { id: id.to_owned(), name: id, url: name.to_owned(), path: "youtube/".to_owned(), source: EmoteSource::Youtube, ..Default::default() });
              },
              _ => ()
            }
          }
        }
      },
      IncomingMessage::RoomId { channel, room_id } => {
        if let Some(sco) = self.channels.get_mut(&channel) && let Channel::Twitch { twitch, shared } = sco {
          twitch.room_id = Some(room_id.to_owned());
          match self.emote_loader.tx.try_send(EmoteRequest::TwitchBadgeEmoteListRequest { 
            channel_id: room_id, 
            channel_name: shared.channel_name.to_owned(),
            token: self.auth_tokens.twitch_auth_token.to_owned(), 
            force_redownload: false
          }) {
            Ok(_) => {},
            Err(e) => warn!("Failed to request channel badge and emote list for {} due to error: {:?}", &channel, e)
          };
          //t.badge_emotes = emotes::twitch_get_channel_badges(&self.auth_tokens.twitch_auth_token, &sco.roomid, &self.emote_loader.base_path, true);
          //info!("loaded channel badges for {}:{}", channel, sco.roomid);
        }
      },
      IncomingMessage::EmoteSets { provider,  emote_sets } => {
        if provider == ProviderName::Twitch {
          if let Some(provider) = self.providers.get_mut(&provider) {
            provider.my_emote_sets = emote_sets;
            for set in &provider.my_emote_sets {
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
        }
      },
      IncomingMessage::UserJoin { channel, username, display_name } => {
        if let Some(c) = self.channels.get_mut(&channel) {
          // Usernames may have completely different display names (e.g. Japanese character display names)
          if display_name.to_lowercase() != username.to_lowercase() {
            c.shared_mut().users.insert(display_name.to_lowercase(), ChannelUser {
              username: username.to_owned(),
              display_name: display_name.to_owned(),
              is_active: true
            });
          }
          c.shared_mut().users.insert(username.to_lowercase(), ChannelUser {
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
            c.shared_mut().users.insert(display_name.to_lowercase(), ChannelUser {
              username: username.to_owned(),
              display_name: display_name.to_owned(),
              is_active: false
            });
          }
          c.shared_mut().users.insert(username.to_lowercase(), ChannelUser {
            username,
            display_name,
            is_active: false
          });
        }
      },
      IncomingMessage::UserMuted { channel, username } => {
        if let Some(history) = self.chat_histories.get_mut(&channel) {
          for (msg, _) in history.iter_mut() {
            if msg.username == username {
              msg.is_removed = Some("<message deleted>".to_string());
            }
          }
        }
      }
        IncomingMessage::VoteStart {  } => {},
        IncomingMessage::VoteStop {  } => {},
    };
  }

  pub fn get_possible_emotes(&mut self, selected_channel: Option<&String>, word: Option<&String>, ctx: &Context) -> Option<Vec<(String, Option<OverlayItem>)>> {
    let emote_loader = &mut self.emote_loader;
    if let Some(input_str) = word {
      if input_str.len() < 2  {
        return None;
      }
      let word = &input_str[0..];
      let word_lower = &word.to_lowercase();

      let mut starts_with_emotes : HashMap<String, Option<OverlayItem>> = Default::default();
      let mut contains_emotes : HashMap<String, Option<OverlayItem>> = Default::default();
      // Find similar emotes. Show emotes starting with same string first, then any that contain the string.
      if let Some(channel_name) = selected_channel && let Some(channel) = self.channels.get(channel_name) {
        if let Some(transient) = channel.transient() && let Some(channel_emotes) = transient.channel_emotes.as_ref() {
          for (name, emote) in channel_emotes { // Channel emotes
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              let emote = emote.get_overlay_item(emote_loader, ctx);
              _ = match name_l.starts_with(word_lower) {
                true => starts_with_emotes.try_insert(name.to_owned(), Some(emote)),
                false => contains_emotes.try_insert(name.to_owned(), Some(emote)),
              };
            }
          }
        }
        if let Some(provider) = self.providers.get(&channel.provider()) { // Provider emotes
          for name in provider.my_sub_emotes.iter() {
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              if let Some(emote) = provider.emotes.get(name) {
                let emote = emote.get_overlay_item(emote_loader, ctx);
                _ = match name_l.starts_with(word_lower) {
                  true => starts_with_emotes.try_insert(name.to_owned(), Some(emote)),
                  false => contains_emotes.try_insert(name.to_owned(), Some(emote)),
                };
              }
            }
          }
        }
        // Global emotes, only if not DGG
        if channel.provider() != ProviderName::DGG {
          for (name, emote) in &self.global_emotes { 
            let name_l = name.to_lowercase();
            if name_l.starts_with(word_lower) || name_l.contains(word_lower) {
              let emote = emote.get_overlay_item(emote_loader, ctx);
              _ = match name_l.starts_with(word_lower) {
                true => starts_with_emotes.try_insert(name.to_owned(), Some(emote)),
                false => contains_emotes.try_insert(name.to_owned(), Some(emote)),
              };
            }
          }
        }
      }
      
      let mut starts_with = starts_with_emotes.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).sorted_by_key(|x| x.0.to_lowercase()).collect_vec();
      let mut contains = contains_emotes.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_owned()).sorted_by_key(|x| x.0.to_lowercase()).collect_vec();
      starts_with.append(&mut contains);
      Some(starts_with)
    }
    else {
      None
    }
  }

  pub fn get_possible_users(&self, selected_channel: Option<&String>, word: Option<&String>) -> Option<Vec<(String, Option<OverlayItem>)>> {
    if let Some(input_str) = word {
      if input_str.len() < 3  {
        return None;
      }
      let word = &input_str[1..];
      let word_lower = &word.to_lowercase();

      let mut starts_with_users : HashMap<String, Option<OverlayItem>> = Default::default();
      let mut contains_users : HashMap<String, Option<OverlayItem>> = Default::default();
      
      if let Some(channel_name) = selected_channel && let Some(channel) = self.channels.get(channel_name) {
        for (name_lower, user) in channel.shared().users.iter().filter(|(_k, v)| v.is_active) {
          if name_lower.starts_with(word_lower) || name_lower.contains(word_lower) {
            _ = match name_lower.starts_with(word_lower) {
              true => starts_with_users.try_insert(user.display_name.to_owned(), None),
              false => contains_users.try_insert(user.display_name.to_owned(), None),
            };
          }
        }
      }
      
      let mut starts_with = starts_with_users.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_lowercase()).collect_vec();
      let mut contains = contains_users.into_iter().map(|x| (x.0, x.1)).sorted_by_key(|x| x.0.to_lowercase()).collect_vec();
      starts_with.append(&mut contains);
      Some(starts_with)
    }
    else {
      None
    }
  }
}
