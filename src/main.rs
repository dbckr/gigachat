/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#![windows_subsystem = "windows"]

use gigachat::TemplateApp;
use tracing::{error};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{Layer, Registry};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_unwrap::{ResultExt};

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
  eframe::run_native(&title, native_options, Box::new(|cc| { 
    cc.egui_ctx.set_fonts(gigachat::ui::load_font());
    let runtime = tokio::runtime::Runtime::new().expect_or_log("new tokio Runtime");
    let mut app = TemplateApp::new(cc, runtime);
    let loader = &mut app.emote_loader;
    match loader.tx.try_send(gigachat::emotes::EmoteRequest::GlobalEmoteListRequest { force_redownload: false }) {  
      Ok(_) => {},
      Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
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
    .with_filter(tracing::level_filters::LevelFilter::INFO)
    .boxed();

  let subscriber = Registry::default().with(console).with(file);
  tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default tracing subscriber");

  guard
}