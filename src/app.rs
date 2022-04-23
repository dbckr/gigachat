/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::{HashMap, HashSet}, borrow::BorrowMut, vec::IntoIter, ops::Add};
use ::egui::Style;
use image::DynamicImage;
use irc::proto::chan;
use tokio::{sync::mpsc, task::JoinHandle};

use chrono::{self, Timelike, DateTime, Utc};
use eframe::{egui::{self, emath, RichText, FontSelection}, epi, epaint::{Color32, text::{LayoutJob, TextWrapping}, FontFamily, FontId, TextureHandle}, emath::{Align, Rect, Pos2}};

use crate::{provider::{twitch, convert_color, ChatMessage, InternalMessage, OutgoingMessage, Channel, Provider, UserProfile, ProviderName, youtube}, emotes::{Emote, EmoteLoader, EmoteStatus, EmoteRequest, EmoteResponse}};
use itertools::Itertools;

const EMOTE_HEIGHT : f32 = 42.0;

pub struct AddChannelMenu {
  channel_name: String,
  channel_id: String,
  auth_token: String,
  provider: ProviderName,
}

impl Default for AddChannelMenu {
    fn default() -> Self {
        Self { 
          channel_name: Default::default(), 
          channel_id: Default::default(), 
          auth_token: Default::default(), 
          provider: ProviderName::Twitch }
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
  #[cfg_attr(feature = "persistence", serde(skip))]
  runtime: tokio::runtime::Runtime,
  providers: HashMap<ProviderName, Provider>,
  channels: HashMap<String, Channel>,
  selected_channel: Option<String>,
  chat_history: Vec<ChatMessage>,
  draft_message: String,
  add_channel_menu_show: bool,
  add_channel_menu: AddChannelMenu,
  pub global_emotes: HashMap<String, Emote>,
  pub emote_loader: EmoteLoader
}

impl TemplateApp {
  pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
      cc.egui_ctx.set_visuals(egui::Visuals::dark());
      let mut r = Self::default();
      r.emote_loader.transparent_img = Some(crate::emotes::load_image_into_texture_handle(&cc.egui_ctx, &DynamicImage::from(image::ImageBuffer::from_pixel(112, 112, image::Rgba::<u8>([0, 0, 0, 0]) ))));
      r
  }
}

impl Default for TemplateApp {
  fn default() -> Self {
    let runtime = tokio::runtime::Runtime::new().expect("new tokio Runtime");
    let loader = EmoteLoader::new(&runtime);
    Self {
      runtime: runtime,
      providers: HashMap::new(),
      channels: HashMap::new(),
      selected_channel: None,
      chat_history: Default::default(),
      draft_message: Default::default(),
      global_emotes: Default::default(),
      emote_loader: loader,
      add_channel_menu_show: Default::default(), 
      add_channel_menu: Default::default()
    }
  } 
}

impl epi::App for TemplateApp {
  /// Called by the frame work to save state before shutdown.
  /// Note that you must enable the `persistence` feature for this to work.
  #[cfg(feature = "persistence")]
  fn save(&mut self, storage: &mut dyn epi::Storage) {
    epi::set_value(storage, epi::APP_KEY, self);
  }

  /// Called each time the UI needs repainting, which may be many times per second.
  /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
  fn update(&mut self, ctx: &egui::Context, frame: &mut epi::Frame) {
    ctx.set_pixels_per_point(1.0);

    while let Ok(event) = self.emote_loader.rx.try_recv() {
      match event {
        EmoteResponse::GlobalEmoteImageLoaded { name, data } => {
          if let Some(emote) = self.global_emotes.get_mut(&name) {
            emote.data = crate::emotes::load_to_texture_handles(ctx, data);
            emote.duration_msec = match emote.data.as_ref() {
              Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
              _ => 0,
            };
            emote.loaded = EmoteStatus::Loaded;
          }
        },
        EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data } => {
          if let Some(channel) = self.channels.get_mut(&channel_name) && let Some(emote) = channel.channel_emotes.get_mut(&name) {
            emote.data = crate::emotes::load_to_texture_handles(ctx, data);
            emote.duration_msec = match emote.data.as_ref() {
              Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
              _ => 0,
            };
            emote.loaded = EmoteStatus::Loaded;
          }
        },
        EmoteResponse::EmoteSetImageLoaded { name, set_id, provider_name, data } => {
          if let Some(provider) = self.providers.get_mut(&provider_name) 
            && let Some(set) = provider.emote_sets.get_mut(&set_id)
            && let Some(emote) = set.get_mut(&name) {
            emote.data = crate::emotes::load_to_texture_handles(ctx, data);
            emote.duration_msec = match emote.data.as_ref() {
              Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
              _ => 0,
            };
            emote.loaded = EmoteStatus::Loaded;
          }
        },
        EmoteResponse::TwitchMsgEmoteLoaded { name, id, data } => {
          if let Some(provider) = self.providers.get_mut(&ProviderName::Twitch) 
            && let Some(emote) = provider.emotes.get_mut(&name) {
            emote.data = crate::emotes::load_to_texture_handles(ctx, data);
            emote.duration_msec = match emote.data.as_ref() {
              Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
              _ => 0,
            };
            emote.loaded = EmoteStatus::Loaded;
          }
        }
      }
    }

    let mut channel_swap = false;
    let mut styles = egui::Style::default();
    styles.text_styles.insert(
      egui::TextStyle::Small,
      FontId::new(18.0, egui::FontFamily::Proportional));
    styles.text_styles.insert(
      egui::TextStyle::Body,
      FontId::new(24.0, egui::FontFamily::Proportional));
    styles.text_styles.insert(
      egui::TextStyle::Button,
      FontId::new(24.0, egui::FontFamily::Proportional));
    ctx.set_style(styles);

    let mut add_channel = |
        channel_options: &mut AddChannelMenu| -> () {
      let c = match channel_options.provider {
        ProviderName::Twitch => { 
          if self.providers.contains_key(&ProviderName::Twitch) == false {
            self.providers.insert(ProviderName::Twitch, Provider {
                name: "twitch".to_owned(),
                emote_sets: Default::default(),
                emotes: Default::default(),
            });
          }
          twitch::open_channel(channel_options.channel_name.to_owned(), &self.runtime, &mut self.emote_loader, &mut self.providers.get_mut(&ProviderName::Twitch).unwrap())
        },
        ProviderName::YouTube => {
          if self.providers.contains_key(&ProviderName::Twitch) == false {
            self.providers.insert(ProviderName::Twitch, Provider {
                name: "twitch".to_owned(),
                emote_sets: Default::default(),
                emotes: Default::default(),
            });
          }
          youtube::open_channel(channel_options.channel_name.to_owned(), channel_options.channel_id.to_owned(), channel_options.auth_token.to_owned(), &self.runtime)
        }
        ProviderName::Null => Channel {
            channel_name: "null".to_owned(),
            provider: ProviderName::Null,
            history: Vec::default(),
            tx: mpsc::channel::<OutgoingMessage>(32).0,
            rx: mpsc::channel(32).1,
            channel_emotes: Default::default(),
            roomid: "".to_owned(),
            task_handle: None,
            is_live: false
        },
        _ => panic!("invalid provider")
      };

      self.channels.insert(channel_options.channel_name.to_owned(), c);
      *(&mut self.selected_channel) = Some(channel_options.channel_name.to_owned());
      channel_options.channel_name = Default::default();
    };

    if self.add_channel_menu_show {
      egui::Window::new("Add Channel").show(ctx, |ui| {
        ui.horizontal(|ui| {
          ui.label("Provider:");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::Twitch, "Twitch");
          ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::YouTube, "Youtube");
          //ui.selectable_value(&mut self.add_channel_menu_provider, ProviderName::Null, "destiny.gg");
          //ui.selectable_value(&mut self.add_channel_menu_provider, ProviderName::Null, "Null");
        });
        ui.horizontal(|ui| {
          ui.label("Channel Name:");
          let name_input = ui.text_edit_singleline(&mut self.add_channel_menu.channel_name);
          //name_input.request_focus();
          if name_input.has_focus() && ui.input().key_pressed(egui::Key::Enter) {
            add_channel(&mut self.add_channel_menu); 
            self.add_channel_menu_show = false;
          }
        });
        if self.add_channel_menu.provider == ProviderName::YouTube {
          ui.horizontal(|ui| {
            ui.label("Channel ID:");
            ui.text_edit_singleline(&mut self.add_channel_menu.channel_id);
          });
          ui.horizontal(|ui| {
            ui.label("Auth Token:");
            ui.text_edit_singleline(&mut self.add_channel_menu.auth_token);
          });
        }
        
        if ui.button("Add channel").clicked() {
          add_channel(&mut self.add_channel_menu);
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
        let label = RichText::new("All Channels").size(24.0);
        let clbl = ui.selectable_value(&mut self.selected_channel, None, label);
        if clbl.clicked() {
          channel_swap = true;
        }

        for (channel, sco) in (&mut self.channels).iter_mut() {
          while let Ok(x) = sco.rx.try_recv() {
            match x {
              InternalMessage::PrivMsg { message } => {
                //sco.history.insert(sco.history.len(), message)
                self.chat_history.insert(self.chat_history.len(), message)
              },
              InternalMessage::MsgEmotes { emote_ids } => {
                if let Some(provider) = (&mut self.providers).get_mut(&sco.provider) {
                  for (id, name) in emote_ids {
                    if provider.emotes.contains_key(&name) == false {
                      println!("inserted twitch emote: {} {}", name, id);
                      provider.emotes.insert(name.to_owned(), self.emote_loader.get_emote(name, id, "".to_owned(), "generated/twitch/".to_owned(), None));
                    }
                  }
                }
              },
              _ => ()
            };
          }

          let label = RichText::new(format!("{} ({})", channel, sco.history.len()))
          .color(get_provider_color(&sco.provider))
          .size(24.0);
          let clbl = ui.selectable_value(&mut self.selected_channel, Some(channel.to_owned()), label);
          if clbl.clicked() {
            channel_swap = true;
          }
          else if clbl.middle_clicked() { //TODO: custom widget that adds close button?
            self.runtime.block_on(async {
              sco.close().await;
            });
            channel_removed = true;
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
    
    egui::CentralPanel::default().frame(cframe)
    .show(ctx, |ui| {
      ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
        if let Some(sc) = &self.selected_channel.to_owned() {
            let outgoing_msg = egui::TextEdit::multiline(&mut self.draft_message)
              .desired_rows(2)
              .desired_width(ui.available_width())
              .hint_text("Type a message to send")
              .font(egui::TextStyle::Body)
              .show(ui);
            ui.separator();
            if outgoing_msg.response.has_focus() && ui.input().key_down(egui::Key::Enter) && ui.input().modifiers.shift == false && self.draft_message.len() > 0 {
              if let Some(sco) = (&mut self.channels).get_mut(sc) {
                match sco.tx.try_send(OutgoingMessage::Chat { message: (&mut self.draft_message).replace("\n", " ").to_owned() }) {
                  Err(e) => println!("Failed to send message: {}", e), //TODO: emit this into UI
                  _ => ()
                } 
                *(&mut self.draft_message) = String::new();
              }
            }
          }
          egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom()
            .show_viewport(ui, |ui, viewport| {
              self.show_variable_height_rows(ctx, ui, viewport, &self.selected_channel.to_owned());
            });
      });
    });

    ctx.request_repaint();
  }

  fn save(&mut self, _storage: &mut dyn epi::Storage) {}

  fn on_exit_event(&mut self) -> bool {
    true
  }

  fn on_exit(&mut self, _ctx : &eframe::glow::Context) {
    self.emote_loader.tx.try_send(EmoteRequest::Shutdown);
    self.emote_loader.close();
    for channel in self.channels.values_mut() {
      channel.close();
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
  fn show_variable_height_rows(&mut self, ctx: &egui::Context, ui : &mut egui::Ui, viewport: emath::Rect, channel_name: &Option<String>) {
    ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
      ui.style_mut().spacing.item_spacing.x = 5.0;
      let y_min = ui.max_rect().top() + viewport.min.y;
      let y_max = ui.max_rect().top() + viewport.max.y;
      let rect = emath::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
  
      let mut in_view : Vec<(&ChatMessage, HashMap<String, EmoteFrame>, Vec<(f32, Option<usize>)>)> = Vec::default();
      let mut y_pos = 0.0;
      let mut skipped_rows = 0;
      //let channel = self.channels.get_mut(channel_name).expect("missing channel name");

      //for row in &channel.history {
      for row in &self.chat_history {
        if let Some(channel) = channel_name && &row.channel != channel {
          //println!("{} {}", row.channel, channel);
          continue;
        }

        let provider_emotes = self.providers.get_mut(&ProviderName::Twitch).and_then(|p| Some(&mut p.emotes));
        //let channel_emotes = &mut channel.channel_emotes;
        let channel_emotes = self.channels.get_mut(&row.channel).and_then(|c| Some(&mut c.channel_emotes));

        let emotes = get_emotes_for_message(&row, &row.channel, provider_emotes, channel_emotes, &mut self.global_emotes, &mut self.emote_loader);
        let msg_sizing = get_chat_msg_size(ui, &row, &emotes);
        let size_y : f32 = (&msg_sizing).into_iter().map(|(height, _first_word)| height).sum();
        
        if y_pos >= viewport.min.y && y_pos /* + size_y */ <= viewport.max.y {
          in_view.push((&row, emotes, msg_sizing));
        }
        else if in_view.len() == 0 {
          skipped_rows += 1;
        }
        y_pos += size_y;
        y_pos += ui.spacing().item_spacing.y;
      }
      ui.set_height(y_pos);
      ui.skip_ahead_auto_ids(skipped_rows);
      ui.allocate_ui_at_rect(rect, |viewport_ui| {
          for (row, emotes, sizing) in in_view {
            create_chat_message(ctx, viewport_ui, row, &emotes, &mut self.emote_loader, sizing);
          }
      });
    });
  }
}

fn get_emotes_for_message(row: &ChatMessage, channel_name: &str, provider_emotes: Option<&mut HashMap<String, Emote>>, channel_emotes: Option<&mut HashMap<String, Emote>>, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> HashMap<String, EmoteFrame> {
  let mut result : HashMap<String, EmoteFrame> = Default::default();
  for word in row.message.to_owned().split(" ") {
    let emote = 
      if let Some(&mut ref mut channel_emotes) = channel_emotes && let Some(emote) = channel_emotes.get_mut(word) {
        get_texture(emote_loader, emote, EmoteRequest::new_channel_request(emote, channel_name))
      }
      else if let Some(emote) = global_emotes.get_mut(word) {
        get_texture(emote_loader, emote, EmoteRequest::new_global_request(emote))
      }
      else if let Some(&mut ref mut provider_emotes) = provider_emotes && let Some(emote) = provider_emotes.get_mut(word) {
        get_texture(emote_loader, emote, EmoteRequest::new_twitch_msg_emote_request(emote))
      }
      /*else if let Some((set_id, set)) = provider.emote_sets.iter_mut().find(|(key, x)| x.contains_key(word)) && let Some(emote) = set.get_mut(word) {
        get_texture(emote_loader, emote, EmoteRequest::new_emoteset_request(emote, &provider.provider, &set_id))
      }*/
      else {
        None
      };
    if let Some(frame) = emote {
      result.insert(frame.name.to_owned(), frame);
    }
  }

  result
}

fn get_provider_color(provider : &ProviderName) -> Color32 {
  match provider {
    ProviderName::Twitch => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
    ProviderName::YouTube => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
    _ => Color32::default()
  }
}

fn create_chat_message(ctx: &egui::Context, ui: &mut egui::Ui, row: &ChatMessage, emotes: &HashMap<String, EmoteFrame>, emote_loader: &mut EmoteLoader, row_sizes: Vec<(f32, Option<usize>)> ) -> emath::Rect {
  let channel_color = get_provider_color(&row.provider);

  let job = get_chat_msg_header_layoutjob(ui, &row.channel, channel_color, &row.username, &row.timestamp, &row.profile);

  let ui_row = ui.horizontal_wrapped(|ui| {
    let tex = emote_loader.transparent_img.as_ref().unwrap();
    let mut row_sizes_iter = row_sizes.into_iter();
    ui.image(tex, emath::Vec2 { x: 1.0, y: row_sizes_iter.next().unwrap().0 });
    ui.label(job);

    /*let mut label_text : Vec<String> = Vec::default();
    let flush_text = |ui : &mut egui::Ui, vec : &mut Vec<String>| {
      let text = vec.into_iter().join(" ");
      if text.len() > 0 {
        let lbl = egui::Label::new(RichText::new(text).size(26.0));
        ui.add(lbl);
      }
      vec.clear();
    };*/
  
    let mut row_size = row_sizes_iter.next();
    let mut ix : usize = 0;
    for word in row.message.to_owned().split(" ") {
      ix += 1;

      if let Some(next_row) = row_size && let Some(next_row_ix) = next_row.1 && next_row_ix == ix {
        ui.label("\n");
        ui.image(tex, emath::Vec2 { x: 1.0, y: next_row.0 });
        row_size = row_sizes_iter.next();
      }

      let emote = emotes.get(word);
      if let Some(EmoteFrame { id, name: _, texture, path, extension }) = emote {
        //flush_text(ui, &mut label_text);
        ui.image(texture, egui::vec2(texture.size_vec2().x * (EMOTE_HEIGHT / texture.size_vec2().y), EMOTE_HEIGHT)).on_hover_ui_at_pointer(|ui| {
          ui.label(format!("{}\n{}\n{}\n{:?}", word, id, path, extension));
          ui.image(texture, texture.size_vec2());
        });
      }
      else {
        //label_text.push(word.to_owned());
        ui.label(word.to_owned());
      }
    }
    //flush_text(ui, &mut label_text);
  });

  ui_row.response.rect
}

fn get_chat_msg_header_layoutjob(ui: &mut egui::Ui, channel_name: &str, channel_color: Color32, username: &String, timestamp: &DateTime<Utc>, profile: &UserProfile) -> LayoutJob {
    let mut job = LayoutJob {
    wrap: TextWrapping { 
      break_anywhere: false,
      max_width: ui.available_width(),
      ..Default::default()
      },
      ..Default::default()
    };
    job.append(&format!("#{channel_name}"), 0., egui::TextFormat { 
      font_id: FontId::new(18.0, FontFamily::Proportional), 
      color: channel_color.linear_multiply(0.6), 
      valign: Align::Center,
      ..Default::default()
    });
    job.append(&format!("[{}]", timestamp.format("%H:%M")), 4.0, egui::TextFormat { 
      font_id: FontId::new(18.0, FontFamily::Proportional), 
      color: Color32::DARK_GRAY, 
      valign: Align::Center,
      ..Default::default()
    });
      let user = match &profile.display_name {
      Some(x) => x,
      None => username
    };
    job.append(&format!("{}:", user), 8.0, egui::TextFormat { 
      font_id: FontId::new(24.0, FontFamily::Proportional), 
      color: convert_color(&profile.color),
      valign: Align::Center,
      ..Default::default()
    });
    job
}

fn get_chat_msg_size(ui: &mut egui::Ui, row: &ChatMessage, emotes: &HashMap<String, EmoteFrame>) -> Vec<(f32, Option<usize>)> {
  // Use text jobs and emote size data to determine rows and overall height of the chat message when layed out
  let max_width = ui.available_width();
  let mut first_word_ix : Option<usize> = None;
  let mut curr_row_width : f32 = 0.0;
  let mut curr_row_height : f32 = 0.0;
  let mut row_data : Vec<(f32, Option<usize>)> = Default::default();

  let mut job = get_chat_msg_header_layoutjob(ui, &row.channel, Color32::WHITE, &row.username, &row.timestamp, &row.profile);
  let header_rows = &ui.fonts().layout_job(job.clone()).rows;
  for header_row in header_rows.into_iter().take(header_rows.len() - 1) {
    row_data.insert(row_data.len(), (header_row.rect.size().y, None));
  }
  curr_row_width += header_rows.last().unwrap().rect.size().x;

  let mut ix = 0;
  for word in row.message.to_owned().split(" ") {
    ix += 1;
    let rect = if let Some(emote) = emotes.get(word) {
      egui::vec2(emote.texture.size_vec2().x * (EMOTE_HEIGHT / emote.texture.size_vec2().y), EMOTE_HEIGHT)
    } else {
      get_text_rect(ui, word)
    };
    
    if curr_row_width + rect.x <= ui.available_width() {
      curr_row_width += rect.x + ui.spacing().item_spacing.x;
      curr_row_height = curr_row_height.max(rect.y);
    }
    else {
      row_data.insert(row_data.len(), (curr_row_height, first_word_ix));
      curr_row_height = rect.y;
      curr_row_width = rect.x + ui.spacing().item_spacing.x;
      first_word_ix = Some(ix);
    }
  }
  if curr_row_width > 0.0 {
    row_data.insert(row_data.len(), (curr_row_height, first_word_ix));
  }
  row_data
}

fn get_text_rect(ui: &mut egui::Ui, text: &str) -> emath::Vec2 {
  let mut job = LayoutJob {
    wrap: TextWrapping { 
      break_anywhere: false,
      max_width: ui.available_width(),
      ..Default::default()
    },
    ..Default::default()
  };
  job.append(text, 0., egui::TextFormat { 
    font_id: FontId::new(26.0, FontFamily::Proportional), 
    ..Default::default() 
  });
  let galley = ui.fonts().layout_job(job.clone());
  galley.rect.size()
}

pub struct EmoteFrame {
  id: String,
  name: String,
  path: String,
  extension: Option<String>,
  texture: TextureHandle
}

fn get_texture<'a> (emote_loader: &mut EmoteLoader, emote : &'a mut Emote, request : EmoteRequest) -> Option<EmoteFrame>{
  match emote.loaded {
    EmoteStatus::NotLoaded => {
      emote_loader.tx.try_send(request);
      emote.loaded = EmoteStatus::Loading;
      None
    },
    EmoteStatus::Loading => None,
    EmoteStatus::Loaded => {
      let frames_opt = emote.data.as_ref();
      match frames_opt {
        Some(frames) => {
          if emote.duration_msec > 0 {
            let time = chrono::Utc::now();
            let target_progress = (time.second() as u16 * 1000 + time.timestamp_subsec_millis() as u16) % emote.duration_msec;
            let mut progress_msec : u16 = 0;
            let mut result = None;
            for (frame, msec) in frames {
              progress_msec += msec; 
              if progress_msec >= target_progress {
                result = Some(EmoteFrame { texture: frame.to_owned(), id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), extension: emote.extension.to_owned() });
                break;
              }
            }
            result
          }
          else {
            let (frame, _delay) = frames.get(0).unwrap();
            Some(EmoteFrame { texture: frame.to_owned(), id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), extension: emote.extension.to_owned() })
          }
        },
        None => None
      }
    }
  }
}