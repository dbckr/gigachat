/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#![windows_subsystem = "windows"]

use std::env;

use egui::ViewportBuilder;
use gigachat::TemplateApp;
use tracing::error;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{Layer, Registry};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_unwrap::ResultExt;
use tracing_log::LogTracer;

fn main() {
  use eframe::Renderer;

  LogTracer::init_with_filter(tracing_log::log::LevelFilter::Warn).expect("failed to init LogTracer");

  let args: Vec<String> = env::args().collect();

  let _file_guard = init_logging(args);

  let mut native_options = eframe::NativeOptions { 
    //transparent: true, 
    //decorated: true,
    vsync: true,
    renderer: Renderer::Glow,
    ..Default::default()
  };
  native_options.viewport = native_options.viewport.with_transparent(true);

  match eframe::run_native("Gigachat", native_options, Box::new(|cc| { 
    cc.egui_ctx.set_fonts(gigachat::ui::load_font());
    let runtime = tokio::runtime::Runtime::new().expect_or_log("new tokio Runtime");
    let mut app = TemplateApp::new(cc, runtime);
    let loader = &mut app.emote_loader;
    match loader.tx.try_send(gigachat::emotes::EmoteRequest::GlobalEmoteListRequest { force_redownload: false }) {  
      Ok(_) => {},
      Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
    };
    Box::new(app)
  })) {
    Ok(_) => (),
    Err(e) => { error!("Error: {:?}", e); }
  };
}

fn init_logging(args: Vec<String>) -> WorkerGuard {
  let working_dir = std::env::current_dir().ok().unwrap_or_default();

  let file_appender = tracing_appender::rolling::hourly(working_dir, "gigachat.log");
  let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

  let log_level = args.iter().find(|a| a.to_ascii_uppercase().starts_with("--LOG=")).map(|x| match x.to_uppercase().as_str() {
    "TRACE" => tracing::level_filters::LevelFilter::TRACE,
    "DEBUG" => tracing::level_filters::LevelFilter::DEBUG,
    "INFO" => tracing::level_filters::LevelFilter::INFO,
    "WARN" => tracing::level_filters::LevelFilter::WARN,
    "ERROR" => tracing::level_filters::LevelFilter::ERROR,
    _ => tracing::level_filters::LevelFilter::ERROR
  }).unwrap_or(tracing::level_filters::LevelFilter::INFO);

  let file = tracing_subscriber::fmt::layer()
    .with_line_number(true)
    .with_ansi(false)
    .with_writer(non_blocking)
    .with_filter(log_level)
    .boxed();  

  let subscriber = Registry::default().with(file);
  tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default tracing subscriber");
  guard
}