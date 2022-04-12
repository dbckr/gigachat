use std::{iter::Map, collections::HashMap};
use futures::prelude::*;
use irc::client::prelude::*;
use failure;
use tokio::sync::mpsc;

use chrono::{Utc,DateTime};
use eframe::{egui, epi};

#[path = "twitch.rs"] mod twitch;

pub struct ChatMessage {
  channel: String,
  username: String,
  timestamp: DateTime<Utc>,
  message: String
}

impl ChatMessage {
  fn new(_channel : &String, _username : &str, _timestamp : DateTime<Utc>, _message : &String) -> Self {
    Self {
      channel: _channel.to_owned(),
      username: _username.to_owned(),
      timestamp: _timestamp,
      message: _message.to_owned()
    }
  }
}

pub struct Channel {
  label: String,
  history: Vec<ChatMessage>,
  rx: mpsc::Receiver<ChatMessage>
}

impl Channel {
    fn new(_name : &String, _rx : mpsc::Receiver<ChatMessage>) -> Self {
        Self { label: _name.to_string(), history: Vec::new(), rx: _rx }
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
  // Example stuff:
  label: String,

  // this how you opt-out of serialization of a member
  #[cfg_attr(feature = "persistence", serde(skip))]
  value: f32,
  selectedChannel: String,
  channels: HashMap<String, Channel>,
}

impl Default for TemplateApp {
  fn default() -> Self {
    Self {
      // Example stuff:
      label: "Hello World!".to_owned(),
      value: 2.7,
      selectedChannel: String::new(),
      channels: HashMap::new()
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
      label,
      value,
      selectedChannel,
      channels
    } = self;

    // Examples of how to create different panels and windows.
    // Pick whichever suits you.
    // Tip: a good default choice is to just keep the `CentralPanel`.
    // For inspiration and more examples, go to https://emilk.github.io/egui

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      // The top panel is often a good place for a menu bar:
      egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
          if ui.button("Configure Tokens").clicked() {}
          if ui.button("Add channel").clicked() {
            let c = twitch::open_channel(&"jormh".to_owned());
            channels.insert("jormh".to_owned(), c);
            *selectedChannel = "jormh".to_owned();
          }
          if ui.button("Quit").clicked() {
            frame.quit();
          }
        });
      });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
      // The central panel the region left after adding TopPanel's and SidePanel's

      let selectedChannelObject = channels.get_mut(&selectedChannel.to_owned());

      if let Some(sco) = selectedChannelObject
      {
        while let Some(cmd) = sco.rx.blocking_recv() {
          sco.history.insert(sco.history.len(), cmd);
        }

        let history = &sco.history;

        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style);
        egui::ScrollArea::vertical().stick_to_bottom().show_rows(
          ui,
          row_height,
          history.len(),
          |ui, row_range| {
            let rows = history.into_iter().skip(row_range.start).take(row_range.count());
            for row in rows {
              let text = format!("{}: {}: {}: {}", row.channel, row.timestamp, row.username, row.message);
              ui.label(text);
            }
          },
        );
      }

      egui::warn_if_debug_build(ui);
    });

    if false {
      egui::Window::new("Window").show(ctx, |ui| {
        ui.label("Windows can be moved by dragging them.");
        ui.label("They are automatically sized based on contents.");
        ui.label("You can turn on resizing and scrolling if you like.");
        ui.label("You would normally chose either panels OR windows.");
      });
    }
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
        egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).into()
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
