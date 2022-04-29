/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use eframe;
use gigachat::TemplateApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use gigachat::provider::ProviderName;

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    ..Default::default() 
  };

  eframe::run_native("Gigachat 0.0", native_options, Box::new(|cc| { 
    let mut app = TemplateApp::new(cc);
    let loader = &mut app.emote_loader;
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
      Err(x) => { println!("ERROR LOADING GLOBAL EMOTES: {}", x); () }
    };
    let b = Box::new(app); 
    b
  }));
}
