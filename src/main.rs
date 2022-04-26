/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use eframe;
use gigachat::TemplateApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use gigachat::app::AuthTokens;

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    ..Default::default() 
  };

  eframe::run_native("Gigachat 0.0", native_options, Box::new(|cc| { 
    let mut app = TemplateApp::new(cc);
    /*app.auth_tokens = AuthTokens {
        twitch_auth_token: cc.storage.and_then(|x| x.get_string("twitch_auth_token")).or_else(|| Some("".to_owned()) ).unwrap(),
        youtube_auth_token: cc.storage.and_then(|x| x.get_string("youtube_auth_token")).or_else(|| Some("".to_owned()) ).unwrap(),
    };*/
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
