#![feature(let_chains)]
use eframe;
mod app;
pub mod provider;
pub mod emotes;
pub use app::TemplateApp;
pub mod test;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::collections::HashMap;

    //use gigachat::TemplateApp;

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    ..Default::default() 
  };

  eframe::run_native("Gigachat 0.0", native_options, Box::new(|cc| { 
    let mut app = TemplateApp::new(cc);
    let loader = &mut app.emote_loader;
    let emotes = &mut app.global_emotes;
    match loader.load_global_emotes() {
      Ok(x) => {
        for (name, emote) in x {
          emotes.insert(name, emote);
        }
      },
      Err(x) => { println!("ERROR LOADING GLOBAL EMOTES: {}", x); () }
    };
    let b = Box::new(app); 
    b
  }));
}
