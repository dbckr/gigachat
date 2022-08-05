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

fn main() {
    use eframe::Renderer;

  let _guard = init_logging();

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    vsync: true,
    renderer: Renderer::Wgpu,
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