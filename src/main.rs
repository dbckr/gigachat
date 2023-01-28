/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#![windows_subsystem = "windows"]

use std::env;
use std::io::BufWriter;

use gigachat::TemplateApp;
use tracing::{error};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{Layer, Registry};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_unwrap::{ResultExt};
use tracing_flame::FlushGuard;
use tracing_flame::FlameLayer;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
  #[cfg(feature = "dhat-heap")]
  let _profiler = dhat::Profiler::new_heap();

  use eframe::Renderer;

  let args: Vec<String> = env::args().collect();

  let (_file_guard, _flame_guard) = init_logging(args);

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    vsync: true,
    renderer: Renderer::Wgpu,
    ..Default::default() 
  };

  eframe::run_native("Gigachat", native_options, Box::new(|cc| { 
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

fn init_logging(args: Vec<String>) -> (WorkerGuard, Option<FlushGuard<BufWriter<std::fs::File>>>) {
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

  let flame_guard = if cfg!(instrumentation) {
    let (flame_layer, flame_guard) = FlameLayer::with_file("./flamelayer.output").unwrap();
    let subscriber = Registry::default().with(file).with(flame_layer);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default tracing subscriber");
    Some(flame_guard)
  }
  else {
    let subscriber = Registry::default().with(file);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default tracing subscriber");
    None
  };

  (guard, flame_guard)
}