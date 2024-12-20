/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use tracing_unwrap::OptionExt;
use std::{collections::{HashMap, VecDeque, vec_deque::IterMut}, iter::Peekable};
use chrono::Utc;
use egui::{emath::Rect, epaint::FontId, TextStyle, TextureHandle};
use egui::{Vec2, FontDefinitions, FontData, text::LayoutJob, FontFamily, Color32};
use itertools::Itertools;
use crate::{
    emotes::{Emote, OverlayItem}, provider::{channel::{Channel, ChannelUser}, dgg, ChatMessage, ComboCounter, Provider, ProviderName
    }};
use crate::emotes::imaging::load_file_into_buffer;

use super::{consts::MIN_LINE_HEIGHT, SelectorFormat, TemplateApp, UiChatMessage, UiChatMessageRow};

pub fn update_font_sizes(r: &TemplateApp, ctx: &egui::Context) {
    let mut styles = egui::Style::default();
    styles.text_styles.insert(
      egui::TextStyle::Small,
      FontId::new(11.0, egui::FontFamily::Proportional));
    styles.text_styles.insert(
      egui::TextStyle::Body,
      FontId::new(r.body_text_size, egui::FontFamily::Proportional));
    styles.text_styles.insert(
      egui::TextStyle::Button,
      FontId::new(14.0, egui::FontFamily::Proportional));
    ctx.set_style(styles);
}

impl eframe::App for TemplateApp {
  #[cfg(feature = "persistence")]
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, eframe::APP_KEY, self);
  }

  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.update_inner(ctx);
  }

  fn on_exit(&mut self, _ctx : Option<&eframe::glow::Context>) {
    self.emote_loader.close();
    if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() {
      chat_mgr.close();
    }
    for (_, channel) in self.channels.iter_mut() {
      channel.close();
    }
  }

  fn auto_save_interval(&self) -> std::time::Duration {
      std::time::Duration::from_secs(30)
  }

//   fn max_size_points(&self) -> eframe::egui::Vec2 {
//     eframe::egui::Vec2::new(1024.0, 2048.0)
//   }

  fn clear_color(&self, _visuals : &eframe::egui::Visuals) -> [f32;4] {
    egui::Color32::from_rgba_unmultiplied(20, 20, 20, 80).to_normalized_gamma_f32()
  }

//   fn persist_native_window(&self) -> bool {
//     true
//   }

  fn persist_egui_memory(&self) -> bool {
    true
  }

//   fn warm_up_enabled(&self) -> bool {
//     false
//   }
}

pub fn get_emote_rects<'a>(
  ui : &egui::Ui,
  ctx : &egui::Context,
  emote_height: f32,
  max_rect: &egui::Rect,
  emotes : &'a [(String, Option<OverlayItem>)],
  format: &SelectorFormat
) -> VecDeque<(egui::Rect, egui::Rect, &'a String, Option<&'a TextureHandle>)> {

    let mut y = max_rect.bottom();
    let mut x = max_rect.left();
    let mut emote_options : VecDeque<(egui::Rect, egui::Rect, &String, Option<&TextureHandle>)> = Default::default();

    for emote in emotes.iter() {
      let text_width = if *format != SelectorFormat::EmoteOnly /*|| emote.1.as_ref().and_then(|f| f.texture).is_none()*/ {
          let mut job = LayoutJob {
              wrap: egui::epaint::text::TextWrapping { 
                break_anywhere: false,
                ..Default::default()
              },
              first_row_min_height: ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT),
              ..Default::default()
            };

            job.append(&emote.0.to_owned(), 0., egui::TextFormat { 
              font_id: get_body_text_style(ctx),
              ..Default::default() });
            let galley = ui.fonts(|f| f.layout_job(job));
            galley.rows.iter().map(|r| r.rect.width()).next().unwrap_or(16.) + 16.
      } else {
          0.
      };

      let margin = Vec2::new(0., 0.);
      let padding = Vec2::new(1., 1.);

      let width = if let Some(ovrl) = &emote.1 && let Some(texture) = ovrl.texture {
        let width = texture.size_vec2().x * (emote_height / texture.size_vec2().y);
        if x + width + text_width > max_rect.right() {
          if y - emote_height * 3. < max_rect.top() {
            break;
          }
          y -= emote_height + padding.y * 2. + margin.y;
          x = max_rect.left();
        }
        width
      } else {
        if x + text_width > max_rect.right() {
          if y - emote_height * 3. < max_rect.top() {
            break;
          }
          y -= emote_height + padding.y * 2. + margin.y;
          x = max_rect.left();
        }
        0.
      };

      let emote_bg_rect = egui::Rect {
        min: egui::pos2(x, y - emote_height - padding.y * 2.),
        max: egui::pos2(x + width + text_width + padding.x * 2., y),
      };

      let emote_img_rect = egui::Rect { 
        min: egui::pos2(x + padding.x, y - emote_height - padding.y), 
        max: egui::pos2(x + width + padding.x, y - padding.y) 
      };

      emote_options.push_back((emote_bg_rect, emote_img_rect, &emote.0, emote.1.as_ref().and_then(|f| f.texture)));

      x = x + width + text_width + padding.x * 2. + margin.x;
    }

    emote_options
  }


pub fn get_body_text_style(ctx: &egui::Context) -> FontId {
    TextStyle::resolve(&TextStyle::Body, ctx.style().as_ref())
}

pub fn get_text_style(text_style: TextStyle, ctx: &egui::Context) -> FontId {
  text_style.resolve(ctx.style().as_ref())
}

pub fn create_uichatmessage<'a,'b>(
  row: &'a ChatMessage,
  ui: &egui::Ui, 
  show_channel_name: bool,
  show_timestamp: bool,
  show_muted: bool,
  providers: &'b HashMap<ProviderName, Provider>,
  channels: &'b HashMap<String, Channel>,
  global_emotes: &'b HashMap<String, Emote>
) -> UiChatMessage<'a,'b> {
  let (provider_emotes, provider_badges) = providers.get(&row.provider)
    .map(|p| (Some(&p.emotes), p.global_badges.as_ref())).unwrap_or((None, None));
  let (channel_emotes, channel_badges) = channels.get(&row.channel)
    .and_then(|c| c.transient())
    .map(|t| (t.channel_emotes.as_ref(), t.badge_emotes.as_ref())).unwrap_or((None, None));

  let emotes = get_emotes_for_message(row, provider_emotes, channel_emotes, global_emotes);
  let (badges, user_color) = get_badges_for_message(row.profile.badges.as_ref(), &row.channel, provider_badges, channel_badges);
  //let ui_width = ui.available_width() - ui.spacing().item_spacing.x;
  //let msg_sizing = chat_estimate::get_chat_msg_size(ui, ui_width, row, &emotes, badges.as_ref(), show_channel_name, show_timestamp, show_muted);
  let mentions = if let Some(channel) = channels.get(&row.channel) {
    get_mentions_in_message(row, &channel.shared().users)
  } else { None };

  let color = row.profile.color.or(user_color).map(|f| f.to_owned());
  let mut row_data : Vec<UiChatMessageRow> = Default::default();
  //for (row_height, msg_char_range, is_ascii_art) in msg_sizing {
  //  row_data.push(UiChatMessageRow { row_height, msg_char_range, is_visible: true, is_ascii_art });
  //}
  //let msg_height = row_data.iter().map(|f| f.row_height).sum();
  let msg_height = 0.;

  UiChatMessage {
    message: row,
    emotes,
    badges,
    mentions,
    row_data,
    msg_height,
    user_color: color,
    show_channel_name,
    show_timestamp
  }
}

pub fn set_selected_message(set_selected_msg: Option<ChatMessage>, ui: &egui::Ui, selected_msg: &mut Option<(Vec2, ChatMessage)>) {
    let mut area = Rect::NOTHING;
    let mut clicked = false;
    if let Some(x) = set_selected_msg.as_ref() {
      let pos = ui.ctx().pointer_hover_pos().unwrap_or_log().to_vec2();
      *selected_msg = Some((Vec2 { x: pos.x, y: pos.y - ui.clip_rect().min.y}, x.to_owned()));
    }
    if let Some((pos, msg)) = selected_msg.as_ref() {
      (area, clicked) = msg_context_menu(ui, pos, msg);
    }
    if clicked || set_selected_msg.is_none() && ui.input(|i| i.pointer.any_click()) && ui.ctx().pointer_interact_pos().is_some() && !area.contains(ui.ctx().pointer_interact_pos().unwrap_or_log()) {
      *selected_msg = None;
    }
}

pub fn msg_context_menu(ui: &egui::Ui, point: &Vec2, msg: &ChatMessage) -> (Rect, bool) {
  let mut clicked = false;
  let window = egui::Window::new("ContextMenu")
  .anchor(egui::Align2::LEFT_TOP, point.to_owned())
  .title_bar(false)
  .show(ui.ctx(), |ui| {
    ui.spacing_mut().item_spacing.x = 4.0;
    let chat_area = egui::ScrollArea::vertical()
      .auto_shrink([true, true])
      .stick_to_bottom(true);
    chat_area.show_viewport(ui, |ui, _viewport| {  
      if ui.button("Copy Message").clicked() {
        ui.output_mut(|o| o.copied_text = msg.message.to_owned());
        clicked = true;
      }
    });
  });
  (window.unwrap_or_log().response.rect, clicked)
}

pub fn push_history(chat_history: &mut VecDeque<(ChatMessage, Option<f32>)>, mut message: ChatMessage, provider_emotes: Option<&HashMap<String, Emote>>, channel_emotes: Option<&HashMap<String, Emote>>, global_emotes: &HashMap<String, Emote>) {
  let is_emote = !get_emotes_for_message(&message, provider_emotes, channel_emotes, global_emotes).is_empty();
  let last = chat_history.iter_mut().rev().find_or_first(|f| f.0.channel == message.channel);
  if let Some(last) = last && is_emote {
    let combo = combo_calculator(&message, last.0.combo_data.as_ref());
    if combo.as_ref().is_some_and(|c| !c.is_new && c.count > 1) && let Some(last_combo) = last.0.combo_data.as_mut() {
      last_combo.is_end = false; // update last item to reflect the continuing combo
    }
    else if last.0.combo_data.as_ref().is_some_and(|c| c.count <= 1) {
      last.0.combo_data = None;
    }
    message.combo_data = combo;
  } 
  else if is_emote {
    let combo = combo_calculator(&message, None);
    message.combo_data = combo;
  }
  chat_history.push_back((message, None));
}

pub fn combo_calculator(row: &ChatMessage, last_combo: Option<&ComboCounter>) -> Option<ComboCounter> { 
  if let Some(last_combo) = last_combo && last_combo.word == row.message.trim() {
    let mut users = last_combo.users.clone();
    
    users.push(row.get_username_with_color().map(|(a,b)| (a.to_owned(), b)).unwrap_or((String::default(), Color32::GRAY)).to_owned());
    Some(ComboCounter {
        word: last_combo.word.to_owned(),
        count: last_combo.count + 1,
        is_new: false,
        is_end: true,
        users
    })
  }
  else if row.message.trim().contains(' ') {
    None
  }
  else {
    Some(ComboCounter {
      word: row.message.trim().to_owned(),
      count: 1,
      is_new: true,
      is_end: true,
      users: [ row.get_username_with_color().map(|(a,b)| (a.to_owned(), b)).unwrap_or((String::default(), Color32::GRAY)).to_owned() ].to_vec()
    })
  }
}

pub fn get_mentions_in_message(row: &ChatMessage, users: &HashMap<String, ChannelUser>) -> Option<Vec<String>> {
  Some(row.message.split(' ').filter_map(|f| {
    let word = f.trim_start_matches('@').trim_end_matches(',').to_lowercase();
    users.get(&word).map(|u| u.display_name.to_owned())
  }).collect_vec())
}

pub fn get_emotes_for_message<'a>(row: &ChatMessage, provider_emotes: Option<&'a HashMap<String, Emote>>, channel_emotes: Option<&'a HashMap<String, Emote>>, global_emotes: &'a HashMap<String, Emote>) -> HashMap<String, &'a Emote> {
  let mut result : HashMap<String, &Emote> = Default::default();
  let results = row.message.to_owned().split(' ').filter_map(|word| {
    if let Some(channel_emotes) = channel_emotes && let Some(emote) = channel_emotes.get(word) {
      Some(emote)
    }
    else if row.provider != ProviderName::DGG && let Some(emote) = global_emotes.get(word) {
      Some(emote)
    }
    else if let Some(provider_emotes) = provider_emotes && let Some(emote) = provider_emotes.get(word) {
      match row.provider {
        ProviderName::Twitch => Some(emote),
        ProviderName::YouTube => Some(emote),
        _ => None
      }
    }
    else {
      None
    }
  }).collect_vec();

  for emote in results {
    result.insert(emote.name.to_owned(), emote);
  }

  result
}

pub fn get_badges_for_message<'a>(badges: Option<&Vec<String>>, channel_name: &str, global_badges: Option<&'a HashMap<String, Emote>>, channel_badges: Option<&'a HashMap<String, Emote>>) -> (Option<Vec<&'a Emote>>, Option<(u8,u8,u8)>) {
  let mut result : Vec<&'a Emote> = Default::default();
  if badges.is_none() { return (None, None); }
  let mut greatest_badge : Option<(isize, (u8,u8,u8))> = None;
  for badge in badges.unwrap_or_log() {
    let emote = 
      if let Some(channel_badges) = channel_badges && let Some(emote) = channel_badges.get(badge) {
        if channel_name == dgg::DGG_CHANNEL_NAME {
          if emote.color.is_some() && (greatest_badge.is_none() || greatest_badge.is_some_and(|b| b.0 > emote.priority)) {
            greatest_badge = Some((emote.priority, emote.color.unwrap_or_log()))
          }
          if emote.hidden {
            continue;
          }
        }
        Some(emote)
        //chat::get_texture(emote_loader, emote, EmoteRequest::new_channel_badge_request(emote, channel_name))
      }
      else if let Some(global_badges) = global_badges && let Some(emote) = global_badges.get(badge) {
        //chat::get_texture(emote_loader, emote, EmoteRequest::new_global_badge_request(emote))
        Some(emote)
      }
      else {
        //EmoteFrame { id: badge.to_owned(), name: badge.to_owned(), label: None, path: badge.to_owned(), texture: None, zero_width: false }
        None
      };
    
    if let Some(emote) = emote {
      result.push(emote);
    } 
  }

  (Some(result), greatest_badge.map(|x| x.1))
}

pub fn load_font() -> FontDefinitions {
  let mut fonts = FontDefinitions::default();

  // Windows, use Segoe
  if let Some(font_file) = load_file_into_buffer("C:\\Windows\\Fonts\\segoeui.ttf") {
    let font = FontData::from_owned(font_file);
    fonts.font_data.insert("Segoe".into(), font);
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Segoe".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "Segoe".into());

    if let Some(emojis_font) = load_file_into_buffer("C:\\Windows\\Fonts\\seguiemj.ttf") {
      let emojis = FontData::from_owned(emojis_font);
      fonts.font_data.insert("emojis".into(), emojis);
      fonts.families.entry(FontFamily::Proportional).or_default().insert(1, "emojis".into());
      fonts.families.entry(FontFamily::Monospace).or_default().insert(1, "emojis".into());
    }

    // More windows specific fallback fonts for extended characters
    if let Some(symbols_font) = load_file_into_buffer("C:\\Windows\\Fonts\\seguisym.ttf") {
      let symbols = FontData::from_owned(symbols_font);
      fonts.font_data.insert("symbols".into(), symbols);
      fonts.families.entry(FontFamily::Proportional).or_default().push("symbols".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("symbols".into());
    }
    // Japanese
    if let Some(jp_font) = load_file_into_buffer("C:\\Windows\\Fonts\\simsunb.ttf.ttf") {
      let jp = FontData::from_owned(jp_font);
      fonts.font_data.insert("SimSun".into(), jp);
      fonts.families.entry(FontFamily::Proportional).or_default().push("SimSun".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("SimSun".into());
    }
    // Amogus
    if let Some(nirmala_font) = load_file_into_buffer("C:\\Windows\\Fonts\\Nirmala.ttf") {
      let nirmala = FontData::from_owned(nirmala_font);
      fonts.font_data.insert("Nirmala".into(), nirmala);
      fonts.families.entry(FontFamily::Proportional).or_default().push("Nirmala".into());
      fonts.families.entry(FontFamily::Monospace).or_default().push("Nirmala".into());
    }
  }
  // Non-windows, check for some linux fonts
  else if let Some(font_file) = load_file_into_buffer("/usr/share/fonts/noto/NotoSans-Regular.ttf") {
    let font = FontData::from_owned(font_file);
    fonts.font_data.insert("NotoSans".into(), font);
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "NotoSans".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "NotoSans".into());
  }
  else if let Some(font_file) = load_file_into_buffer("/usr/share/fonts/TTF/OpenSans-Regular.ttf") {
    let font = FontData::from_owned(font_file);
    fonts.font_data.insert("OpenSans".into(), font);
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "OpenSans".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "OpenSans".into());
  }

  fonts
}

pub fn mentioned_in_message(usernames: &HashMap<ProviderName, String>, provider: &ProviderName, message : &str) -> bool {
  if let Some(username) = usernames.get(provider) {
    message.split(' ').map(|f| {
      f.trim_start_matches('@').trim_end_matches(',').to_lowercase()
    }).any(|f| username == &f)
  } else {
    false
  }
}

pub fn get_provider_color(provider : &ProviderName) -> Color32 {
    match provider {
      //ProviderName::Twitch => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
      ProviderName::Twitch => Color32::from_rgba_unmultiplied(169, 112, 255, 255),
      ProviderName::YouTube => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
      ProviderName::DGG => Color32::from_rgba_unmultiplied(83, 140, 198, 255),
    }
  }

pub fn convert_color(input : &(u8, u8, u8)) -> Color32 {
    // return white
    if input == &(255u8, 255u8, 255u8) {
      return Color32::WHITE;
    }
  
    // normalize brightness
    let target = 150;
   
    let min = |x, y| -> u8 {
      let z = x < y;
      match z {
        true => x,
        _ => y
      }
    };
  
    let tf = |x| -> (u8, u8) {
      if x < target {
        (target - x, 255 - x)
      }
      else {
        (0, 255 - x)
      }
    };
  
    let (r, g, b) = (input.0, input.1, input.2);
  
    let (r_diff, r_max_adj) = tf(r);
    let (g_diff, g_max_adj) = tf(g);
    let (b_diff, b_max_adj) = tf(b);
  
    let adj = ((r_diff as u16 + g_diff as u16 + b_diff as u16) / 3) as u8;
  
    let (rx, gx, bx) = (r + min(adj, r_max_adj), g + min(adj, g_max_adj), b + min(adj, b_max_adj));
  
    //info!("{} {} {}", rx, gx, bx);
    Color32::from_rgb(rx, gx, bx)
  }