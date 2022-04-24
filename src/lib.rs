#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), deny(warnings))]
#![feature(let_chains)]
#![feature(is_some_with)]
#![feature(map_try_insert)]

pub mod app;
pub mod provider;
pub mod emotes;
pub mod test;
pub use app::TemplateApp;

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    let app = TemplateApp::default();
    eframe::start_web(canvas_id, Box::new(app))
}