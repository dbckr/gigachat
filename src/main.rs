/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use gigachat::TemplateApp;
use gigachat::provider::ProviderName;
use gigachat::error_util::{LogErrResult, LogErrOption};
use tracing::{info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{Layer, Registry};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

#[cfg(feature = "use-bevy")]
use gigachat::ui;

#[cfg(all(not(feature = "use-bevy"), not(target_arch = "wasm32")))]
fn main() {
  let _guard = init_logging();

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    vsync: true,
    ..Default::default() 
  };

  let title = format!("Gigachat - {}", env!("CARGO_PKG_VERSION"));
  eframe::run_native(&title.to_owned(), native_options, Box::new(|cc| { 
    cc.egui_ctx.set_fonts(gigachat::ui::load_font());
    let runtime = tokio::runtime::Runtime::new().log_expect("new tokio Runtime");
    let mut app = TemplateApp::new(cc, title, runtime);
    let loader = app.emote_loader.as_ref().log_unwrap();
    let emotes = &mut app.global_emotes;
    if let Some(twitch) = app.providers.get_mut(&ProviderName::Twitch) {
      twitch.global_badges = loader.twitch_get_global_badges(&app.auth_tokens.twitch_auth_token)
    }
    match loader.load_global_emotes() {
      Ok(x) => {
        for (name, emote) in x {
          emotes.insert(name, emote);
        }
      },
      Err(x) => { info!("ERROR LOADING GLOBAL EMOTES: {}", x); }
    };
    Box::new(app)
  }));
}

#[cfg(feature = "use-bevy")]
fn main() {
    use bevy::window::WindowDescriptor;

  let _guard = init_logging();

  let title = format!("Gigachat - {}", env!("CARGO_PKG_VERSION"));
  let runtime = tokio::runtime::Runtime::new().log_expect("new tokio Runtime");
  let mut app = TemplateApp::new(title, runtime);
  let loader = app.emote_loader.as_ref().log_unwrap();
  let emotes = &mut app.global_emotes;
  if let Some(twitch) = app.providers.get_mut(&ProviderName::Twitch) {
    twitch.global_badges = loader.twitch_get_global_badges(&app.auth_tokens.twitch_auth_token)
  }
  match loader.load_global_emotes() {
    Ok(x) => {
      for (name, emote) in x {
        emotes.insert(name, emote);
      }
    },
    Err(x) => { info!("ERROR LOADING GLOBAL EMOTES: {}", x); }
  };

  bevy::prelude::App::new()
    .insert_resource(bevy::core_pipeline::ClearColor(bevy::prelude::Color::rgba(0.0, 0.0, 0.0, 120.0)))
    .insert_resource(bevy::prelude::Msaa { samples: 2 })
    // Optimal power saving and present mode settings for desktop apps.
    .insert_resource(bevy::winit::WinitSettings::desktop_app())
    .insert_resource(WindowDescriptor {
        present_mode: bevy::window::PresentMode::Mailbox,
        ..Default::default()
    })
    .insert_resource::<TemplateApp>(app)
    .add_plugins(bevy::DefaultPlugins)
    .add_plugin(bevy_framepace::FramepacePlugin::default())
    .add_plugin(bevy_egui::EguiPlugin)
    .add_startup_system(ui::bevy_configure_visuals)
    .add_system(ui::bevy_update_ui_scale_factor)
    .add_system(ui::bevy_update)
    .run();
}

fn init_logging() -> WorkerGuard {
  let working_dir = std::env::current_dir().ok().unwrap_or_default();

  let file_appender = tracing_appender::rolling::hourly(working_dir, "gigachat.log");
  let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

  let console = tracing_subscriber::fmt::layer()
    .with_line_number(true)
    .boxed();

  let file = tracing_subscriber::fmt::layer()
    .with_line_number(true)
    .with_ansi(false)
    .with_writer(non_blocking)
    .boxed();

  let subscriber = Registry::default().with(console).with(file);
  tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default tracing subscriber");

  guard
}