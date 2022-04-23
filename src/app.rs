/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::{HashMap, HashSet}, borrow::BorrowMut};
use tokio::{sync::mpsc, task::JoinHandle};

use chrono::{self, Timelike};
use eframe::{egui::{self, emath, RichText}, epi, epaint::{Color32, text::{LayoutJob, TextWrapping}, FontFamily, FontId, TextureHandle}, emath::{Align, Rect, Pos2}};

use crate::{provider::{twitch, convert_color, ChatMessage, InternalMessage, OutgoingMessage, Channel, Provider}, emotes::{Emote, EmoteLoader, EmoteStatus, EmoteRequest, EmoteResponse}};
use itertools::Itertools;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
  #[cfg_attr(feature = "persistence", serde(skip))]
  runtime: tokio::runtime::Runtime,
  providers: HashMap<String, Provider>,
  channels: HashMap<String, Channel>,
  selected_channel: Option<String>,
  draft_message: String,
  add_channel_menu_show: bool,
  add_channel_menu_channel_name: String,
  add_channel_menu_provider: String,
  pub global_emotes: HashMap<String, Emote>,
  pub emote_loader: EmoteLoader
}

impl TemplateApp {
  pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
      cc.egui_ctx.set_visuals(egui::Visuals::dark());
      Self::default()
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
      draft_message: Default::default(),
      add_channel_menu_show: false,
      add_channel_menu_channel_name: Default::default(),
      add_channel_menu_provider: "twitch".to_owned(),
      global_emotes: Default::default(),
      emote_loader: loader
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
    let Self {
      runtime,
      providers,
      channels,
      selected_channel,
      draft_message,
      add_channel_menu_show,
      add_channel_menu_channel_name,
      add_channel_menu_provider,
      global_emotes,
      emote_loader
    } = self;

    ctx.set_pixels_per_point(1.0);

    while let Ok(event) = emote_loader.rx.try_recv() {
      match event {
        EmoteResponse::GlobalEmoteImageLoaded { name, data } => {
          if let Some(emote) = global_emotes.get_mut(&name) {
            emote.data = crate::emotes::load_to_texture_handles(ctx, data);
            emote.duration_msec = match emote.data.as_ref() {
              Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
              _ => 0,
            };
            emote.loaded = EmoteStatus::Loaded;
          }
        },
        EmoteResponse::ChannelEmoteImageLoaded { name, channel_name, data } => {
          if let Some(channel) = channels.get_mut(&channel_name) && let Some(emote) = channel.channel_emotes.get_mut(&name) {
            emote.data = crate::emotes::load_to_texture_handles(ctx, data);
            emote.duration_msec = match emote.data.as_ref() {
              Some(framedata) => framedata.into_iter().map(|(_, delay)| delay).sum(),
              _ => 0,
            };
            emote.loaded = EmoteStatus::Loaded;
          }
        },
        EmoteResponse::EmoteSetImageLoaded { name, set_id, provider_name, data } => {
          if let Some(provider) = providers.get_mut(&provider_name) 
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
          if let Some(provider) = providers.get_mut("twitch") 
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

    let mut add_channel = |show_toggle: &mut bool, channel_name_input: &mut String, provider_input: &mut String| -> () {
      let c = match provider_input.as_str() {
        "twitch" => { 
          if providers.contains_key("twitch") == false {
            providers.insert("twitch".to_owned(), Provider {
                provider: "twitch".to_owned(),
                emote_sets: Default::default(),
                emotes: Default::default(),
            });
          }
          twitch::open_channel(channel_name_input.to_owned(), &runtime, emote_loader, providers.get_mut("twitch").unwrap())
        },
        "null" => Channel {
            channel_name: "null".to_owned(),
            provider: "null".to_owned(),
            history: Vec::default(),
            history_viewport_size_y: 0.0,
            tx: mpsc::channel::<OutgoingMessage>(32).0,
            rx: mpsc::channel(32).1,
            channel_emotes: Default::default(),
            roomid: "".to_owned(),
            task_handle: None
        },
        _ => panic!("invalid provider")
      };

      channels.insert(channel_name_input.to_owned(), c);
      *selected_channel = Some(channel_name_input.to_owned());
      *channel_name_input = Default::default();
      *show_toggle = false;
    };

    if add_channel_menu_show.to_owned() {
      egui::Window::new("Add Channel").show(ctx, |ui| {
        ui.horizontal(|ui| {
          ui.label("Provider:");
          ui.selectable_value(add_channel_menu_provider, "twitch".to_owned(), "Twitch");
          ui.selectable_value(add_channel_menu_provider, "youtube".to_owned(), "Youtube");
          ui.selectable_value(add_channel_menu_provider, "dgg".to_owned(), "destiny.gg");
          ui.selectable_value(add_channel_menu_provider, "null".to_owned(), "Null");
        });
        ui.horizontal(|ui| {
          ui.label("Channel Name:");
          let name_input = ui.text_edit_singleline(add_channel_menu_channel_name);
          name_input.request_focus();
          if name_input.has_focus() && ui.input().key_pressed(egui::Key::Enter) {
            add_channel(add_channel_menu_show, add_channel_menu_channel_name, add_channel_menu_provider); 
          }
        });
        if ui.button("Add channel").clicked() {
          add_channel(add_channel_menu_show, add_channel_menu_channel_name, add_channel_menu_provider);
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
            if ui.button("Add channel").clicked() {
              *add_channel_menu_show = true;
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
        for (channel, sco) in channels.iter_mut() {
          loop {
            if sco.provider != "null" {
              match sco.rx.try_recv() {
                Ok(x) => {
                  match x {
                    InternalMessage::PrivMsg { message } => sco.history.insert(sco.history.len(), message),
                    InternalMessage::MsgEmotes { emote_ids } => {
                      if let Some(provider) = providers.get_mut(&sco.provider) {
                        for (id, name) in emote_ids {
                          if provider.emotes.contains_key(&name) == false {
                            println!("inserted twitch emote: {} {}", name, id);
                            provider.emotes.insert(name.to_owned(), emote_loader.get_emote(name, id, "".to_owned(), "generated/twitch/".to_owned(), None));
                          }
                        }
                      }
                    },
                    _ => ()
                  };
                },
                Err(_) => break,
              };
            }
          }

          let label = RichText::new(format!("{} ({})", channel, sco.history.len())).size(24.0);
          let clbl = ui.selectable_value(selected_channel, Some(channel.to_owned()), label);
          if clbl.clicked() {
            channel_swap = true;
          }
          else if clbl.middle_clicked() { //TODO: custom widget that adds close button?
            runtime.block_on(async {
              sco.close().await;
            });
            channel_removed = true;
          }
        }
      });
      if channel_removed {
        if let Some(name) = selected_channel {
          channels.remove(name);
        }
        *selected_channel = None;
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
        if let Some(sc) = selected_channel {
          if let Some(sco) = channels.get_mut(&sc.to_owned()) {
            let outgoing_msg = egui::TextEdit::multiline(draft_message)
              .desired_rows(2)
              .desired_width(ui.available_width())
              .hint_text("Type a message to send")
              .font(egui::TextStyle::Body)
              .show(ui);
              ui.separator();
              ui.add_space(15.0);
            if outgoing_msg.response.has_focus() && ui.input().key_down(egui::Key::Enter) && ui.input().modifiers.shift == false && draft_message.len() > 0 {
              match sco.tx.try_send(OutgoingMessage::Chat { message: draft_message.replace("\n", " ").to_owned() }) {
                Err(e) => println!("Failed to send message: {}", e), //TODO: emit this into UI
                _ => ()
              } 
              *draft_message = String::new();
            }
            egui::ScrollArea::vertical()
              .auto_shrink([false; 2])
              .stick_to_bottom()
              .show_viewport(ui, |ui, viewport| {
                show_variable_height_rows(ctx, ui, viewport, sco, providers.get_mut(&sco.provider).unwrap(), global_emotes, emote_loader);
              });
          }
        }
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
    // NOTE: a bright gray makes the shadows of the windows look weird.
    // We use a bit of transparency so that if the user switches on the
    // `transparent()` option they get immediate results.
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

fn show_variable_height_rows(ctx: &egui::Context, ui : &mut egui::Ui, viewport : emath::Rect, sco: &mut Channel, provider: &mut Provider, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) {
  ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
    let y_min = ui.max_rect().top() + viewport.min.y;
    let y_max = ui.max_rect().top() + viewport.max.y;
    //println!("{} {}", y_min, y_max);
    let rect = emath::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);

    let mut temp_dbg_sizes : Vec<f32> = Vec::default();
    let mut in_view : Vec<&ChatMessage> = Vec::default();
    let allowed_y = viewport.max.y - viewport.min.y;
    let mut used_y = 0.0;
    let mut y_pos = 0.0;
    let mut skipped_rows = 0;
    for row in &sco.history {
      let size_y = get_y_size(ui, sco, row, global_emotes);
      
      if y_pos >= viewport.min.y && y_pos <= viewport.max.y && used_y + size_y <= allowed_y {
        temp_dbg_sizes.push(size_y);
        in_view.push(row);
        used_y += size_y;
      }
      else if in_view.len() == 0 {
        skipped_rows += 1;
      }
      y_pos += size_y;
    }

    ui.set_height(viewport.max.y);
    ui.skip_ahead_auto_ids(skipped_rows);

    let mut last_rect : Rect = Rect { min: Pos2{ x: 0.0, y: 0.0 }, max: Pos2{ x: 0.0, y: 0.0 } };
    ui.allocate_ui_at_rect(rect, |viewport_ui| {
      for row in in_view {
        last_rect = create_chat_message(ctx, viewport_ui, provider, &sco.channel_name, &mut sco.channel_emotes, row, global_emotes, emote_loader);
        temp_dbg_sizes.push(last_rect.height());
      }
    });
  });
}

fn create_chat_message(ctx: &egui::Context, ui: &mut egui::Ui, provider: &mut Provider, channel_name: &str, channel_emotes: &mut HashMap<String, Emote>, row: &ChatMessage, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> emath::Rect {
  let channel_color = match provider.provider.as_str() {
    "twitch" => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
    "youtube" => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
    _ => Color32::default()
  };

  let mut job = LayoutJob {
    wrap: TextWrapping { 
      break_anywhere: false,
      max_width: ui.available_width(),
      ..Default::default()
    },
    //first_row_min_height: row_height,
    ..Default::default()
  };
  job.append(channel_name, 0., egui::TextFormat { 
    font_id: FontId::new(18.0, FontFamily::Proportional), 
    color: channel_color, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&format!("[{}]", row.timestamp.format("%H:%M:%S")), 4.0, egui::TextFormat { 
    font_id: FontId::new(18.0, FontFamily::Proportional), 
    color: Color32::DARK_GRAY, 
    valign: Align::Center,
    ..Default::default()
  });
  let user = match &row.profile.display_name {
    Some(x) => x,
    None => &row.username
  };
  job.append(&format!("{}:", user), 8.0, egui::TextFormat { 
    font_id: FontId::new(24.0, FontFamily::Proportional), 
    color: convert_color(&row.profile.color),
    valign: Align::Center,
    ..Default::default()
  });

  let ui_row = ui.horizontal_wrapped(|ui| {
    ui.label(job);
    let mut label_text : Vec<String> = Vec::default();

    let flush_text = |ui : &mut egui::Ui, vec : &mut Vec<String>| {
      let text = vec.into_iter().join(" ");
      if text.len() > 0 {
        let lbl = egui::Label::new(RichText::new(text).size(26.0));
        ui.add(lbl);
      }
      vec.clear();
    };
  
    for word in row.message.to_owned().split(" ") {
      let emote = 
        if let Some(emote) = channel_emotes.get_mut(word) {
          get_texture(emote_loader, emote, EmoteRequest::new_channel_request(emote, channel_name))
        }
        else if let Some(emote) = global_emotes.get_mut(word) {
          get_texture(emote_loader, emote, EmoteRequest::new_global_request(emote))
        }
        else if let Some(emote) = provider.emotes.get_mut(word) {
          get_texture(emote_loader, emote, EmoteRequest::new_twitch_msg_emote_request(emote))
        }
        /*else if let Some((set_id, set)) = provider.emote_sets.iter_mut().find(|(key, x)| x.contains_key(word)) && let Some(emote) = set.get_mut(word) {
          get_texture(emote_loader, emote, EmoteRequest::new_emoteset_request(emote, &provider.provider, &set_id))
        }*/
        else {
          None
        };

      if let Some(EmoteFrame { id, name: _, texture, path, extension }) = emote {
        flush_text(ui, &mut label_text);
          ui.image(&texture, egui::vec2(texture.size_vec2().x * (42. / texture.size_vec2().y), 42.)).on_hover_ui_at_pointer(|ui| {
            ui.label(format!("{}\n{}\n{}\n{:?}", word, id, path, extension));
            ui.image(&texture, texture.size_vec2());
          });
      }
      else {
        label_text.push(word.to_owned());
      }
    }
    flush_text(ui, &mut label_text);
  });

  ui_row.response.rect
}



fn get_y_size(ui: &mut egui::Ui, sco: &Channel, row: &ChatMessage, global_emotes: &HashMap<String, Emote>) -> f32 {
  let channel_color = match sco.provider.as_str() {
    "twitch" => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
    "youtube" => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
    _ => Color32::default()
  };

  let mut job = LayoutJob {
    wrap: TextWrapping { 
      break_anywhere: false,
      max_width: ui.available_width(),
      ..Default::default()
    },
    //first_row_min_height: row_height,
    ..Default::default()
  };
  job.append(&sco.channel_name, 0., egui::TextFormat { 
    font_id: FontId::new(18.0, FontFamily::Proportional), 
    color: channel_color, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&format!("[{}]", row.timestamp.format("%H:%M:%S")), 4.0, egui::TextFormat { 
    font_id: FontId::new(18.0, FontFamily::Proportional), 
    color: Color32::DARK_GRAY, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&row.username.to_owned(), 8.0, egui::TextFormat { 
    font_id: FontId::new(24.0, FontFamily::Proportional), 
    color: convert_color(&row.profile.color),
    valign: Align::Center,
    ..Default::default()
  });

  for word in row.message.to_owned().split(" ") {
    if global_emotes.contains_key(word) {
      job.append("IMAGE ", 0.0, egui::TextFormat { 
        font_id: FontId::new(42.0, FontFamily::Proportional),
        valign: Align::Center,
        ..Default::default()
      }); 
    }
    else if sco.channel_emotes.contains_key(word) {
      job.append("IMAGE ", 0.0, egui::TextFormat { 
        font_id: FontId::new(24.0, FontFamily::Proportional),
        valign: Align::Center,
        ..Default::default()
      }); 
    }
    else {
      job.append(&format!("{} ", word), 0.0, egui::TextFormat { 
        font_id: FontId::new(24.0, FontFamily::Proportional),
        valign: Align::Center,
        ..Default::default()
      }); 
    }
  }

  let galley = ui.fonts().layout_job(job.clone());
  
  match galley.size().y {
    //x if x > 16.0 => x - 16.0,
    x => x
  }
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