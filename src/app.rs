use std::{collections::HashMap};
use tokio::sync::mpsc;

use chrono::{Utc,DateTime};
use eframe::{egui::{self, Label, emath, InnerResponse}, epi, epaint::{Color32, text::{LayoutJob, TextWrapping}, FontFamily, FontId}};

use crate::provider::{twitch, convert_color};

#[derive(Clone)]
pub struct UserBadge {
  pub image_data: Vec<u8>
}

#[derive(Clone)]
pub struct UserProfile {
  pub badges: Vec<UserBadge>,
  pub display_name: String,
  pub color: (u8, u8, u8)
}


impl Default for UserProfile {
  fn default() -> Self {
    Self {
      color: (255, 255, 255),
      display_name: Default::default(),
      badges: Vec::new()
    }
  }
}

pub struct ChatMessageUI {
  pub data: ChatMessage,
  pub viewport_size_y: f32,
  //pub ui: egui::Label
}

#[derive(Clone)]
pub struct ChatMessage {
  pub username: String,
  pub timestamp: DateTime<Utc>,
  pub message: String,
  pub profile: UserProfile 
}

impl Default for ChatMessage {
  fn default() -> Self {
    Self {
      username: Default::default(),
      timestamp: Utc::now(),
      message: Default::default(),
      profile: Default::default()
    }
  }
}

pub struct Channel {
  pub label: String,
  pub provider: String,
  pub history: Vec<ChatMessage>,
  pub history_viewport_size_y: f32,
  pub rx: mpsc::Receiver<ChatMessage>
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
  add_channel_menu_provider: String
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
      add_channel_menu_provider: "twitch".to_owned()
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
      add_channel_menu_provider
    } = self;

    let mut add_channel = |show_toggle: &mut bool, channel_name_input: &mut String, provider_input: &mut String| -> () {
      let c = match provider_input.as_str() {
        "twitch" => twitch::open_channel(channel_name_input.to_owned(), &runtime),
        "null" => Channel {
            label: "null".to_owned(),
            provider: "null".to_owned(),
            history: Vec::default(),
            history_viewport_size_y: 0.0,
            rx: mpsc::channel(32).1
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
                  sco.history.insert(sco.history.len(), x);
                },
                Err(_) => break,
              };
            }
          }

          let label = format!("{} ({})", channel, sco.history.len());
          ui.selectable_value(selected_channel, Some(channel.to_owned()), label.to_owned());
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
                show_variable_height_rows(ui, viewport, sco);
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

fn show_variable_height_rows(ui : &mut egui::Ui, viewport : emath::Rect, sco: &Channel) -> InnerResponse<()> {
  let y_min = ui.max_rect().top() + viewport.min.y;
  let y_max = ui.max_rect().top() + viewport.max.y;
  let rect = emath::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);

  let mut temp_dbg_sizes : Vec<f32> = Vec::default();
  let mut labels : Vec<Label> = Vec::default();
  let allowed_y = viewport.max.y - viewport.min.y;
  let mut used_y = 0.0;
  let mut y_pos = 0.0;
  let mut ix = 0;
  let mut skipped_rows = 0;
  let mut last_size = 0.0;
  for row in &sco.history {
    ix += 1;
    let (size_y, job) = create_chat_message(ui, sco, row, last_size as u32);
    last_size = size_y;
    
    if y_pos >= viewport.min.y && y_pos <= viewport.max.y /*&& used_y + size_y <= allowed_y*/ {
      temp_dbg_sizes.push(size_y);
      labels.push(Label::new(job));
      used_y += size_y;
    }
    if labels.len() == 0 {
      skipped_rows += 1;
    }
    y_pos += size_y;
  }

  ui.set_height(y_pos);
  //print!("{} {}", y_pos, ui.min_size().y);
  ui.skip_ahead_auto_ids(skipped_rows); // Make sure we get consistent IDs.

  let mut add_contents = |ui : &mut egui::Ui, row_range : std::ops::Range<f32>| {
    for lbl in labels {
      temp_dbg_sizes.push(ui.add(lbl).rect.height());
    }
  };

  let r = ui.allocate_ui_at_rect(rect, |viewport_ui| {
    add_contents(viewport_ui,  viewport.min.y..viewport.max.y)
  });
  //println!(" {}", ui.min_size().y);
  r
}

fn create_chat_message(ui: &mut egui::Ui, sco: &Channel, row: &ChatMessage, ix: u32) -> (f32, LayoutJob) {
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
    ..Default::default()
  });
  job.append(&sco.label, 0., egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: channel_color, 
    ..Default::default()
  });
  job.append(&format!("[{}]", row.timestamp.format("%H:%M:%S")), 4.0, egui::TextFormat { 
    font_id: FontId::new(12.0, FontFamily::Proportional), 
    color: Color32::DARK_GRAY, 
    ..Default::default()
  });
  job.append(&row.username.to_owned(), 8.0, egui::TextFormat { 
    font_id: FontId::new(16.0, FontFamily::Proportional), 
    color: convert_color(&row.profile.color),
    ..Default::default()
  });
  job.append(&row.message.to_owned(), 8.0, egui::TextFormat { 
    font_id: FontId::new(16.0, FontFamily::Proportional),
    ..Default::default()
  });

  let galley = ui.fonts().layout_job(job.clone());
  
  (
    match galley.size().y {
      //x if x > 16.0 => x - 16.0,
      x => x
    },
    job
  )
}