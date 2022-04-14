use std::{collections::HashMap};
use tokio::sync::mpsc;

use chrono::{Utc,DateTime};
use eframe::{egui::{self, Label}, epi, epaint::{Color32, text::LayoutJob, FontFamily, FontId}};

use crate::provider::{twitch, convert_color};

pub struct UserBadge {
  pub image_data: Vec<u8>
}

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
  fn name(&self) -> &str {
    "Gigachat 0.0"
  }

  /// Called once before the first frame.
  fn setup(
    &mut self,
    _ctx: &egui::Context,
    _frame: &epi::Frame,
    _storage: Option<&dyn epi::Storage>,
  ) {
    // Load previous app state (if any).
    // Note that you must enable the `persistence` feature for this to work.
    #[cfg(feature = "persistence")]
    if let Some(storage) = _storage {
      *self = epi::get_value(storage, epi::APP_KEY).unwrap_or_default()
    }
  }

  /// Called by the frame work to save state before shutdown.
  /// Note that you must enable the `persistence` feature for this to work.
  #[cfg(feature = "persistence")]
  fn save(&mut self, storage: &mut dyn epi::Storage) {
    epi::set_value(storage, epi::APP_KEY, self);
  }

  /// Called each time the UI needs repainting, which may be many times per second.
  /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
  fn update(&mut self, ctx: &egui::Context, frame: &epi::Frame) {
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
      let mut c = match provider_input.as_str() {
        "twitch" => twitch::open_channel(channel_name_input.to_owned(), &runtime),
        _ => panic!("invalid provider")
      };
      c.history.insert(c.history.len(), ChatMessage { 
        username: "server".to_owned(), 
        timestamp: chrono::Utc::now(), 
        message: format!("Added channel."),
        profile: UserProfile::default()
      });
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
            match sco.rx.try_recv() {
              Ok(x) => {
                println!("{}", x.message);
                sco.history.insert(sco.history.len(), x)
              },
              Err(_) => break,
            };
          }

          let label = format!("{} ({})", channel, sco.history.len());
          ui.selectable_value(selected_channel, Some(channel.to_owned()), label.to_owned());
        }
      });

      egui::warn_if_debug_build(ui);
    });

    let cframe = egui::Frame { 
      margin: egui::style::Margin::same(5.0), 
      fill: egui::Color32::from(egui::Color32::TRANSPARENT),
      ..Default::default() 
    };
    egui::CentralPanel::default().frame(cframe).show(ctx, |ui| {
      ui.vertical(|ui| {
        if let Some(sc) = selected_channel {
          if let Some(sco) = channels.get(&sc.to_owned()) {
            let history = &sco.history;

            let row_height = 16.0;
            egui::ScrollArea::vertical()
              .max_height(ui.available_height() - 50.)  
              .auto_shrink([false; 2])
              .stick_to_bottom()
              .show_rows( // needs a way to account for wrapped rows
                ui,
                row_height,
                history.len(),
                |ui, row_range| {
                  let rows = history.into_iter().skip(row_range.start).take(row_range.count());
                  for row in rows {
                    let channel_color = match sco.provider.as_str() {
                      "twitch" => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
                      "youtube" => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
                      _ => Color32::default()
                    };
                    
                    let mut job = LayoutJob {
                      wrap_width: ui.available_width() * 0.8,
                      //first_row_min_height: row_height,
                      ..Default::default()
                    };

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

                    let lbl = Label::new(job).wrap(true);
                    ui.add(lbl);
                  }
                },
              );
              ui.separator();
              ui.text_edit_singleline(draft_message);
              ui.label(chrono::Utc::now().to_string());
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

  fn on_exit(&mut self) {}

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
    egui::Color32::from_rgba_unmultiplied(12, 12, 12, 200).into()
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
