use std::{collections::HashMap};
use tokio::sync::mpsc;

use chrono;
use eframe::{egui::{self, emath, InnerResponse}, epi, epaint::{Color32, text::{LayoutJob, TextWrapping}, FontFamily, FontId, ColorImage, TextureHandle}, emath::{Align, Rect, Pos2}};

use crate::{provider::{twitch, convert_color, ChatMessage, InternalMessage}, emotes::{Emote, EmoteLoader}};
use itertools::Itertools;

pub struct Channel {
  pub channel_name: String,
  pub roomid: String,
  pub provider: String,
  pub history: Vec<ChatMessage>,
  pub history_viewport_size_y: f32,
  pub rx: mpsc::Receiver<InternalMessage>,
  pub channel_emotes: HashMap<String, Emote>
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
  #[cfg_attr(feature = "persistence", serde(skip))]
  runtime: tokio::runtime::Runtime,
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
      // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
      // Restore app state using cc.storage (requires the "persistence" feature).
      // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
      // for e.g. egui::PaintCallback.
      cc.egui_ctx.set_visuals(egui::Visuals::dark());
      Self::default()
  }
}

impl Default for TemplateApp {
  fn default() -> Self {
    Self {
      runtime: tokio::runtime::Runtime::new().expect("new tokio Runtime"),
      channels: HashMap::new(),
      selected_channel: None,
      draft_message: Default::default(),
      add_channel_menu_show: false,
      add_channel_menu_channel_name: Default::default(),
      add_channel_menu_provider: "twitch".to_owned(),
      global_emotes: Default::default(),
      emote_loader: Default::default()
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
      channels,
      selected_channel,
      draft_message,
      add_channel_menu_show,
      add_channel_menu_channel_name,
      add_channel_menu_provider,
      global_emotes,
      emote_loader
    } = self;

    let mut channel_swap = false;

    let mut add_channel = |show_toggle: &mut bool, channel_name_input: &mut String, provider_input: &mut String| -> () {
      let c = match provider_input.as_str() {
        "twitch" => twitch::open_channel(channel_name_input.to_owned(), &runtime, emote_loader),
        "null" => Channel {
            channel_name: "null".to_owned(),
            provider: "null".to_owned(),
            history: Vec::default(),
            history_viewport_size_y: 0.0,
            rx: mpsc::channel(32).1,
            channel_emotes: Default::default(),
            roomid: "".to_owned(),
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
      // The top panel is often a good place for a menu bar:
      egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
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

      ui.horizontal(|ui| {
        for (channel, sco) in channels.iter_mut() {
          loop {
            if sco.provider != "null" {
              match sco.rx.try_recv() {
                Ok(x) => {
                  match x {
                    InternalMessage::PrivMsg { message } => sco.history.insert(sco.history.len(), message),
                    _ => ()
                  };
                },
                Err(_) => break,
              };
            }
          }

          let label = format!("{} ({})", channel, sco.history.len());
          if ui.selectable_value(selected_channel, Some(channel.to_owned()), label.to_owned()).clicked() {
            channel_swap = true;
          }
        }
      });
    });

    let cframe = egui::Frame { 
      inner_margin: egui::style::Margin::same(5.0), 
      fill: egui::Color32::from(egui::Color32::TRANSPARENT),
      ..Default::default() 
    };
    egui::CentralPanel::default().frame(cframe).show(ctx, |ui| {
      ui.vertical(|ui| {
        if let Some(sc) = selected_channel {
          if let Some(sco) = channels.get_mut(&sc.to_owned()) {
            egui::ScrollArea::vertical()
              .max_height(ui.available_height() - 50.)  
              .auto_shrink([false; 2])
              .stick_to_bottom()
              .show_viewport(ui, |ui, viewport| {
                show_variable_height_rows(ctx, ui, viewport, sco, channel_swap, global_emotes, emote_loader);
              });

            ui.separator();
            ui.label(chrono::Utc::now().to_string());
            ui.text_edit_singleline(draft_message);
            //egui::warn_if_debug_build(ui);

            /*if ui.text_edit_singleline(draft_message).has_focus() && ui.input().key_pressed(egui::Key::Enter) {
              sco.history.insert(sco.history.len(), ChatMessage { 
                username: "bob".to_owned(), 
                timestamp: Utc::now(), 
                message: draft_message.to_owned(), 
                profile: Default::default() 
              });
              draft_message.clear();
            }*/
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

  fn on_exit(&mut self, _ctx : &eframe::glow::Context) {}

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

fn show_variable_height_rows(ctx: &egui::Context, ui : &mut egui::Ui, viewport : emath::Rect, sco: &mut Channel, channel_swap : bool, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> InnerResponse<()> {
  let y_min = ui.max_rect().top() + viewport.min.y;
  let y_max = ui.max_rect().top() + viewport.max.y;
  let rect = emath::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);

  let mut temp_dbg_sizes : Vec<f32> = Vec::default();
  let mut in_view : Vec<(&ChatMessage, u32)> = Vec::default();
  let allowed_y = viewport.max.y - viewport.min.y;
  let mut used_y = 0.0;
  let mut y_pos = 0.0;
  let mut ix = 0;
  //let mut skipped_rows = 0;
  let mut last_size = 0.0;
  for row in &sco.history {
    ix += 1;
    let size_y = get_y_size(ui, sco, row, last_size as u32, global_emotes);
    last_size = size_y;
    
    if y_pos >= viewport.min.y && y_pos <= viewport.max.y /*&& used_y + size_y <= allowed_y*/ {
      temp_dbg_sizes.push(size_y);
      in_view.push((row, ix));
      used_y += size_y;
    }
    y_pos += size_y;
  }

  ui.set_height(y_pos);
  //print!("{} {}", y_pos, ui.min_size().y);
  //ui.skip_ahead_auto_ids(skipped_rows); // Make sure we get consistent IDs.

  let mut last_rect : Rect = Rect { min: Pos2{ x: 0.0, y: 0.0 }, max: Pos2{ x: 0.0, y: 0.0 } };
  let r = ui.allocate_ui_at_rect(rect, |viewport_ui| {
    for (row, ix) in in_view {
      let rect = viewport_ui.horizontal_wrapped(|ui| {      
        last_rect = create_chat_message(ctx, ui, &sco.provider, &sco.channel_name, &mut sco.channel_emotes, row, ix, global_emotes, emote_loader);
        temp_dbg_sizes.push(last_rect.height());
      }).response.rect;
    }
  });

  if channel_swap {
    ui.scroll_to_rect(last_rect, Some(Align::BOTTOM));
  }
  //println!(" {}", ui.min_size().y);
  r
}

fn create_chat_message(ctx: &egui::Context, ui: &mut egui::Ui, provider: &str, channel_name: &str, channel_emotes: &mut HashMap<String, Emote>, row: &ChatMessage, ix: u32, global_emotes: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> emath::Rect {
  let channel_color = match provider {
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
  job.append(&format!("{}",ix), 0., egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: Color32::WHITE,
    valign: Align::Center,
    ..Default::default()
  });
  job.append(channel_name, 0., egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: channel_color, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&format!("[{}]", row.timestamp.format("%H:%M:%S")), 4.0, egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: Color32::DARK_GRAY, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&row.username.to_owned(), 8.0, egui::TextFormat { 
    font_id: FontId::new(16.0, FontFamily::Proportional), 
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
        ui.label(text);
      }
      vec.clear();
    };
  
    for word in row.message.to_owned().split(" ") {
      if global_emotes.contains_key(word) {
        flush_text(ui, &mut label_text);
        if let Some(tex) = get_texture(ctx, word, global_emotes, emote_loader) {
          ui.image(&tex, tex.size_vec2());
        }
        else {
          ui.label(word);
        }
      }
      else if channel_emotes.contains_key(word) {
        flush_text(ui, &mut label_text);
        if let Some(tex) = get_texture(ctx, word, channel_emotes, emote_loader) {
          ui.image(&tex, tex.size_vec2());
        }
        else {
          ui.label(word);
        }
      }
      else {
        label_text.push(word.to_owned());
      }
    }
    flush_text(ui, &mut label_text);
  });

  ui_row.response.rect
}



fn get_y_size(ui: &mut egui::Ui, sco: &Channel, row: &ChatMessage, ix: u32, global_emotes: &HashMap<String, Emote>) -> f32 {
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
  job.append(&format!("{}",ix), 0., egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: Color32::WHITE,
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&sco.channel_name, 0., egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: channel_color, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&format!("[{}]", row.timestamp.format("%H:%M:%S")), 4.0, egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: Color32::DARK_GRAY, 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&row.username.to_owned(), 8.0, egui::TextFormat { 
    font_id: FontId::new(16.0, FontFamily::Proportional), 
    color: convert_color(&row.profile.color),
    valign: Align::Center,
    ..Default::default()
  });

  for word in row.message.to_owned().split(" ") {
    if global_emotes.contains_key(word) {
      job.append("IMAGE ", 0.0, egui::TextFormat { 
        font_id: FontId::new(24.0, FontFamily::Proportional),
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
        font_id: FontId::new(16.0, FontFamily::Proportional),
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

fn get_texture (ctx : &egui::Context, word: &str, dict: &mut HashMap<String, Emote>, emote_loader: &mut EmoteLoader) -> Option<TextureHandle> {
  let emote = dict.get_mut(word).unwrap();
  if emote.loaded == false {
    emote_loader.load_image(ctx, emote);
    emote.loaded = true;
  }

  let img = emote.data.as_ref();
  match img {
    Some(x) => {
      let frame_ix = (60 * chrono::Utc::now().timestamp_subsec_millis() / 1000) as usize % x.len();
      let frame = x.get(frame_ix).unwrap();
      Some(frame.to_owned())
    },
    None => None
  }
}

/*fn get_texture_old (ctx : &egui::Context, word: &str, dict: &HashMap<String, Emote>) -> Option<TextureHandle> {
  let emote = dict.get(word).unwrap();
  let img = emote.data.as_ref();
  match img {
    Some(x) => {
      let frame_ix = (30 * chrono::Utc::now().timestamp_subsec_millis() as usize / 1000) % x.len();
      let frame = x.get(frame_ix as usize).unwrap();
      Some(ctx.load_texture(word.to_owned(), load_image_from_memory(&frame.resize(24, 24, FilterType::Lanczos3))))
    },
    None => None
  }
}*/