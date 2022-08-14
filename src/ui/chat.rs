/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use tracing::info;
use chrono::{Timelike, DateTime, Utc};
use egui::{emath, Rounding};
use egui::{Color32, FontFamily, FontId, Align, RichText, text::LayoutJob, Pos2, TextureHandle};
use itertools::Itertools;
use crate::error_util::{LogErrOption};

use crate::{emotes::*, provider::{ProviderName, UserProfile}};

use super::{SMALL_TEXT_SIZE, BADGE_HEIGHT, BODY_TEXT_SIZE, MIN_LINE_HEIGHT, EMOTE_HEIGHT, UiChatMessage, COMBO_LINE_HEIGHT, chat_estimate::{TextRange}};

pub fn create_combo_message(ui: &mut egui::Ui, row: &UiChatMessage, transparent_img: &TextureHandle, show_channel_name: bool, show_timestamp: bool) -> emath::Rect {
  let channel_color = get_provider_color(&row.message.provider);
  let job = get_chat_msg_header_layoutjob(true, ui, &row.message.channel, channel_color, None, &row.message.timestamp, &row.message.profile, show_channel_name, show_timestamp);
  let ui_row = ui.horizontal_wrapped(|ui| {
    ui.image(transparent_img, emath::Vec2 { x: 1.0, y: COMBO_LINE_HEIGHT });
    ui.label(job);
    //if let Some(combo) = row.combo.as_ref().and_then(|c| if c.is_final { Some(c) } else { None }) &&
    if let Some(combo) = row.message.combo_data.as_ref() {
      let emote = row.emotes.get(&combo.word);
      if let Some(EmoteFrame { id: _, name: _, label: _, texture, path, zero_width }) = emote {
        let texture = texture.as_ref().unwrap_or(transparent_img);
        add_ui_emote_image(&combo.word, path, texture, zero_width, &mut None, ui, COMBO_LINE_HEIGHT - 4.);
      }
      ui.label(RichText::new(format!("{}x combo", combo.count)).size(COMBO_LINE_HEIGHT * 0.6));
    }
  });
  ui_row.response.rect
}

pub fn create_chat_message(ui: &mut egui::Ui, chat_msg: &UiChatMessage, transparent_img: &TextureHandle, highlight: Option<Color32>) -> (emath::Rect, Option<String>) {
  let mut user_selected : Option<String> = None;
  let mut message_color : Option<(u8,u8,u8)> = None;
  if chat_msg.message.provider == ProviderName::DGG && chat_msg.message.message.starts_with('>') {
    message_color =  Some((99, 151, 37));
  }

  let channel_color = get_provider_color(&chat_msg.message.provider);
  let ui_row = ui.horizontal_wrapped(|ui| {
    let mut row_ix = 0;
    if chat_msg.is_ascii_art {
      ui.spacing_mut().item_spacing.y = 0.;
    }

    let chat_msg_rows = chat_msg.row_data.iter().map(|row| {
      match &row.msg_char_range {
        TextRange::Range { range } => (chat_msg.message.message.char_indices().map(|(_i, x)| x).skip(range.start).take(range.end - range.start).collect::<String>(), row.is_visible, row.row_height),
        TextRange::EndRange { range } => (chat_msg.message.message.char_indices().map(|(_i, x)| x).skip(range.start).collect::<String>(), row.is_visible, row.row_height)
      }
    });

    for (message, is_visible, row_height) in chat_msg_rows {
      let mut last_emote_width : Option<(f32, f32)> = None;
      if is_visible {
        ui.image(transparent_img, emath::Vec2 { x: 1.0, y: row_height });
        ui.set_row_height(row_height);

        if let Some(highlight) = highlight {
          highlight_ui_row(ui, highlight);
        } else if chat_msg.message.is_server_msg {
          highlight_ui_row(ui, Color32::from_rgba_unmultiplied(90, 75, 0, 90));
        }

        if row_ix == 0 {
          let uname_text = chat_msg.message.profile.display_name.as_ref().unwrap_or(&chat_msg.message.username);
          let username = if !chat_msg.message.is_server_msg { Some(uname_text) } else { None };
          let job = get_chat_msg_header_layoutjob(true, ui, &chat_msg.message.channel, channel_color, username, &chat_msg.message.timestamp, &chat_msg.message.profile, chat_msg.show_channel_name, chat_msg.show_timestamp);
          ui.label(job);
          if let Some(user_badges) = &chat_msg.message.profile.badges {
            for badge in user_badges {
              let emote = chat_msg.badges.as_ref().and_then(|f| f.get(badge));
              let tex = emote.and_then(|g| g.texture.as_ref()).unwrap_or(transparent_img);
              ui.image(tex, egui::vec2(tex.size_vec2().x * (BADGE_HEIGHT / tex.size_vec2().y), BADGE_HEIGHT)).on_hover_ui(|ui| {
                ui.set_width(BADGE_HEIGHT + 20.);
                ui.vertical_centered(|ui| {
                  ui.image(tex, tex.size_vec2());
                  match chat_msg.message.provider {
                    ProviderName::Twitch => {
                      let parts = badge.split('/').collect_tuple::<(&str, &str)>().unwrap_or(("",""));
                      match parts.0 {
                        "subscriber" => {
                          let num = parts.1.parse::<usize>().unwrap_or(0);
                          let tier = match num / 1000 {
                            3 => "T3",
                            2 => "T2",
                            _ => "T1",
                          };
                          ui.label(format!("{} Month Sub ({})", num % 1000, tier))
                        }, 
                        "sub-gifter" => ui.label(format!("{}\nGift Subs", parts.1)),
                        "bits" => ui.label(format!("{} Bits", parts.1)),
                        _ => ui.label(parts.0)
                      };
                    },
                    ProviderName::DGG => { ui.label(emote.and_then(|x| x.label.as_ref()).unwrap_or(badge)); }
                  };
                });
              });
            }
          }
    
          if !chat_msg.message.is_server_msg {
            let uname_rich_text = RichText::new(&format!("{}:", uname_text))
              .size(BODY_TEXT_SIZE)
              .color(convert_color(chat_msg.user_color.as_ref()));
            let uname = ui.add(egui::Label::new(uname_rich_text).sense(egui::Sense::click()));
            if uname.clicked() {
              user_selected = Some(uname_text.to_lowercase());
            }
            if uname.hovered() {
              ui.ctx().output().cursor_icon = egui::CursorIcon::PointingHand;
            }
          }
        }
        for word in message.split(' ') {
          let link_url = chat_msg.message.message.split_ascii_whitespace().find_or_first(|f| f.contains(word)).and_then(|f| if is_url(f) { Some(f) } else { None });
          let emote = chat_msg.emotes.get(word);
          if let Some(EmoteFrame { id: _, name: _, label: _, texture, path, zero_width }) = emote {
            let tex = texture.as_ref().unwrap_or(transparent_img);
            add_ui_emote_image(word, path, tex, zero_width, &mut last_emote_width, ui, EMOTE_HEIGHT);
          }
          else {
            last_emote_width = None;
            match link_url {
              Some(url) => {
                let link = ui.add(egui::Label::new(RichText::new(word).size(BODY_TEXT_SIZE).color(ui.visuals().hyperlink_color)).sense(egui::Sense::click()));
                if link.hovered() {
                  ui.ctx().output().cursor_icon = egui::CursorIcon::PointingHand;
                }
                if link.clicked() {
                  let modifiers = ui.ctx().input().modifiers;
                  ui.ctx().output().open_url = Some(egui::output::OpenUrl {
                    url: url.to_owned(),
                    new_tab: modifiers.any(),
                  });
                }
              },
              None => {
                let text = match chat_msg.is_ascii_art {
                  true => RichText::new(word).family(FontFamily::Monospace),
                  false => RichText::new(word).color(convert_color(message_color.as_ref()))
                }.size(BODY_TEXT_SIZE);

                if let Some (mention) = chat_msg.mentions.as_ref().and_then(|f| f.iter().find(|m| word.to_lowercase().contains(&m.to_lowercase()))) {
                  let lbl = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                  if lbl.clicked() {
                    user_selected = Some(mention.to_owned());
                  }
                  if lbl.hovered() {
                    ui.ctx().output().cursor_icon = egui::CursorIcon::PointingHand;
                  }
                } else {
                  let lbl = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                  if lbl.clicked() {
                    ui.output().copied_text = chat_msg.message.message.to_owned();
                  }
                }
              }
            };
          }
        }
        ui.end_row();
      }
      row_ix += 1;
    }
  });
  let actual = format!("{:.2}", ui_row.response.rect.size().y + ui.spacing().item_spacing.y);
  let expected = format!("{:.2}", chat_msg.row_data.iter().filter_map(|f| if f.is_visible { Some(f.row_height + ui.spacing().item_spacing.y) } else { None }).sum::<f32>());
  if actual != expected {
    info!("expected {} actual {} for {}", expected, actual, &chat_msg.message.username);
  }
  (ui_row.response.rect, user_selected)
}

fn add_ui_emote_image(word: &str, path: &str, texture: &egui::TextureHandle, zero_width: &bool, last_emote_width: &mut Option<(f32, f32)>, ui: &mut egui::Ui, emote_height: f32) {
  let (x, y) = (texture.size_vec2().x * (emote_height / texture.size_vec2().y), emote_height);
  if *zero_width {
    let (x, y) = last_emote_width.unwrap_or((x, y));
    let img = egui::Image::new(texture, egui::vec2(x, y));
    let cursor = ui.cursor().to_owned();
    let rect = egui::epaint::Rect { min: Pos2 {x: cursor.left() - x - ui.spacing().item_spacing.x, y: cursor.top()}, max:  Pos2 {x: cursor.left() - ui.spacing().item_spacing.x, y: cursor.bottom()} };
    img.paint_at(ui, rect);
  }
  else {
    ui.image(texture, egui::vec2(x, y)).on_hover_ui(|ui| {
      ui.label(format!("{}\n{}", word, path.replace("cache/", "").replace('/',"")));
      ui.image(texture, texture.size_vec2());
    });
    *last_emote_width = Some((x, y));
  }
}

/*fn dim_ui_emote_image(last_emote_width: &Option<(f32, f32)>, ui: &mut egui::Ui, emote_height: f32) {
  if let Some((x, y)) = last_emote_width {
    let cursor = ui.cursor().to_owned();
    let rect = egui::epaint::Rect { 
      min: Pos2 {
        x: cursor.left() - x - ui.spacing().item_spacing.x, 
        y: cursor.top()}, 
      max:  Pos2 {
        x: cursor.left() - ui.spacing().item_spacing.x, 
        y: cursor.bottom()} };
    ui.painter().rect_filled(
      rect, 
      Rounding::none(), 
      Color32::from_rgba_unmultiplied(0, 0, 0, 210));
  }
}*/

fn highlight_ui_row(ui: &mut egui::Ui, color: Color32) {
  let cursor = ui.cursor().to_owned();
  let rect = egui::epaint::Rect { 
    min: Pos2 {
      x: cursor.left(), 
      y: cursor.top()}, 
    max:  Pos2 {
      x: cursor.left() + ui.available_width(), 
      y: cursor.bottom() + ui.spacing().item_spacing.y} };
  ui.painter().rect_filled(
    rect, 
    Rounding::none(), 
    //Color32::from_rgba_unmultiplied(90, 90, 90, 90)
    color
  );
}

fn is_url(word: &str) -> bool {
    //TODO: regex?
    word.starts_with("http")
}

pub fn get_chat_msg_header_layoutjob(for_display: bool, ui: &mut egui::Ui, channel_name: &str, channel_color: Color32, username: Option<&String>, timestamp: &DateTime<Utc>, profile: &UserProfile, show_channel_name: bool, show_timestamp: bool) -> LayoutJob {
  let mut job = LayoutJob {
    break_on_newline: false,
    first_row_min_height: ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT),
    ..Default::default()
  };
  if show_channel_name {
    job.append(&format!("#{channel_name} "), 0., egui::TextFormat { 
        font_id: FontId::new(SMALL_TEXT_SIZE, FontFamily::Proportional), 
        color: channel_color.linear_multiply(0.6), 
        valign: Align::Center,
        ..Default::default()
      });
  }
  if show_timestamp {
    job.append(&format!("[{}] ", timestamp.with_timezone(&chrono::Local).format("%H:%M")), 0., egui::TextFormat { 
      font_id: FontId::new(SMALL_TEXT_SIZE, FontFamily::Proportional), 
      color: Color32::DARK_GRAY, 
      valign: Align::Center,
      ..Default::default()
    });
  }
  if for_display { return job; }

  if let Some(username) = username {
    job.append(&format!("{}:", &profile.display_name.as_ref().unwrap_or(username)), ui.spacing().item_spacing.x, egui::TextFormat {
      font_id: FontId::new(BODY_TEXT_SIZE, FontFamily::Proportional),
      color: convert_color(profile.color.as_ref()),
      valign: Align::Center,
      ..Default::default()
    });
  }
  job
}

pub fn convert_color(input : Option<&(u8, u8, u8)>) -> Color32 {
  // return white
  if input.is_none() || input.is_some_and(|x| x == &&(255u8, 255u8, 255u8)) {
    return Color32::WHITE;
  }
  let input = input.log_unwrap();

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


pub struct EmoteFrame {
  pub id: String,
  pub name: String,
  pub path: String,
  pub label: Option<String>,
  //extension: Option<String>,
  pub texture: Option<egui::TextureHandle>,
  pub zero_width: bool
}

pub fn get_texture(emote_loader: &mut EmoteLoader, emote : &Emote, request : EmoteRequest) -> EmoteFrame {
  match emote.loaded {
    EmoteStatus::NotLoaded => {
      if !emote_loader.loading_emotes.contains_key(&emote.name) {
        if let Err(e) = emote_loader.tx.try_send(request) {
          info!("Error sending emote load request: {}", e);
        }
        emote_loader.loading_emotes.insert(emote.name.to_owned(), chrono::Utc::now());
      }
      EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), label: emote.display_name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width }
    },
    EmoteStatus::Loaded => {
      let frames_opt = emote.data.as_ref();
      match frames_opt {
        Some(frames) => {
          if emote.duration_msec > 0 {
            let time = chrono::Utc::now();
            let target_progress = (time.second() as u16 * 1000 + time.timestamp_subsec_millis() as u16) % emote.duration_msec;
            let mut progress_msec : u16 = 0;
            for (frame, msec) in frames {
              progress_msec += msec; 
              if progress_msec >= target_progress {
                return EmoteFrame { texture: Some(frame.to_owned()), id: emote.id.to_owned(), name: emote.name.to_owned(), label: emote.display_name.to_owned(), path: emote.path.to_owned(), zero_width: emote.zero_width };
              }
            }
            EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), label: emote.display_name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width }
          }
          else {
            let (frame, _delay) = frames.get(0).log_unwrap();
            EmoteFrame { texture: Some(frame.to_owned()), id: emote.id.to_owned(), label: emote.display_name.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), zero_width: emote.zero_width }
          }
        },
        None => EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), label: emote.display_name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width }
      }
    }
  }
}

pub fn get_provider_color(provider : &ProviderName) -> Color32 {
  match provider {
    //ProviderName::Twitch => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
    ProviderName::Twitch => Color32::from_rgba_unmultiplied(169, 112, 255, 255),
    //ProviderName::YouTube => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
    ProviderName::DGG => Color32::from_rgba_unmultiplied(83, 140, 198, 255),
  }
}