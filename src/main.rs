/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use gigachat::TemplateApp;
use gigachat::provider::ProviderName;
use gigachat::ui;

#[cfg(all(not(feature = "use-bevy"), not(target_arch = "wasm32")))]
fn main() {
  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    vsync: true,
    ..Default::default() 
  };

  let title = format!("Gigachat - {}", env!("CARGO_PKG_VERSION"));
  eframe::run_native(&title.to_owned(), native_options, Box::new(|cc| { 
    cc.egui_ctx.set_fonts(gigachat::ui::load_font());
    let runtime = tokio::runtime::Runtime::new().expect("new tokio Runtime");
    let mut app = TemplateApp::new(cc, title, runtime);
    let loader = app.emote_loader.as_ref().unwrap();
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
      Err(x) => { println!("ERROR LOADING GLOBAL EMOTES: {}", x); }
    };
    Box::new(app)
  }));
}

#[cfg(feature = "use-bevy")]
fn main() {
    use bevy::window::WindowDescriptor;

  let title = format!("Gigachat - {}", env!("CARGO_PKG_VERSION"));
  //cc.egui_ctx.set_fonts(gigachat::ui::load_font());
  let runtime = tokio::runtime::Runtime::new().expect("new tokio Runtime");
  let mut app = TemplateApp::new(title, runtime);
  let loader = app.emote_loader.as_ref().unwrap();
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
    Err(x) => { println!("ERROR LOADING GLOBAL EMOTES: {}", x); }
  };

  bevy::prelude::App::new()
    //.insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .insert_resource(bevy::prelude::Msaa { samples: 4 })
    // Optimal power saving and present mode settings for desktop apps.
    //.insert_resource(WinitSettings::desktop_app())
    .insert_resource(WindowDescriptor {
        present_mode: bevy::window::PresentMode::Mailbox,
        ..Default::default()
    })
    .insert_resource::<TemplateApp>(app)
    .add_plugins(bevy::DefaultPlugins)
    .add_plugin(bevy_egui::EguiPlugin)
    //.add_startup_system(configure_visuals)
    //.add_system(update_ui_scale_factor)
    .add_system(ui::bevy_update)
    .run();
}