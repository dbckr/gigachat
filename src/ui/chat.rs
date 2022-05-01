/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::HashMap;

use chrono::{Timelike, DateTime, Utc};
use eframe::{emath, epaint::text::TextWrapping};
use egui::{Color32, FontFamily, FontId, Align, RichText, text::LayoutJob, Vec2, Pos2};
use itertools::Itertools;

use crate::{emotes::*, provider::{ChatMessage, ProviderName, UserProfile}};

use super::{SMALL_TEXT_SIZE, BADGE_HEIGHT, BODY_TEXT_SIZE, MIN_LINE_HEIGHT, EMOTE_HEIGHT, WORD_LENGTH_MAX};

pub fn create_chat_message(ui: &mut egui::Ui, row: &ChatMessage, emotes: &HashMap<String, EmoteFrame>, badges: Option<&HashMap<String, EmoteFrame>>, emote_loader: &mut EmoteLoader, row_sizes: Vec<(f32, Option<usize>)>, row_include: Vec<bool> ) -> emath::Rect {
  let channel_color = get_provider_color(&row.provider);
  let mut row_shown = false;

  let job = get_chat_msg_header_layoutjob(true, ui, &row.channel, channel_color, &row.username, &row.timestamp, &row.profile, badges);

  let ui_row = ui.horizontal_wrapped(|ui| {
    let tex = emote_loader.transparent_img.as_ref().unwrap();
    let mut row_sizes_iter = row_sizes.iter();
    let mut next_row_size = row_sizes_iter.next();
    let mut row_include_iter = row_include.iter();
    let mut should_include_row = row_include_iter.next();

    if let Some(include_row) = should_include_row && *include_row { // showing first row
      ui.image(tex, emath::Vec2 { x: 1.0, y: next_row_size.unwrap().0 });
      ui.label(job);

      if let Some(user_badges) = &row.profile.badges {
        for badge in user_badges {
          let tex = badges.and_then(|f| f.get(badge).and_then(|g| g.texture.as_ref())).or(emote_loader.transparent_img.as_ref());
          if let Some(tex) = tex {
            ui.image(tex, egui::vec2(&tex.size_vec2().x * (BADGE_HEIGHT / &tex.size_vec2().y), BADGE_HEIGHT)).on_hover_ui(|ui| {
              ui.set_width(BADGE_HEIGHT + 20.);
              ui.vertical_centered(|ui| {
                ui.image(tex, tex.size_vec2());
                let parts = badge.split("/").collect_tuple::<(&str, &str)>().unwrap_or(("",""));
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
                  _ => ui.label(format!("{}", parts.0))
                };
              });
            });
          }
          else {
            ui.add_space(BADGE_HEIGHT + ui.spacing().item_spacing.x);
          }
        }
      }

      let uname = egui::Label::new(RichText::new(&format!("{}:", &row.profile.display_name.as_ref().unwrap_or(&row.username))).color(convert_color(&row.profile.color)));
      ui.add(uname);

      row_shown = true;
    } 
    next_row_size = row_sizes_iter.next();  

    //let mut label_text : Vec<String> = Vec::default();
    
    let mut last_emote_width : Option<(f32, f32)> = None;
    let mut ix : usize = 0;
    for word in row.message.to_owned().split(" ") {

      let link_url = is_url(word).then(|| word.to_owned());
      
      let subwords = 
        if word.len() > WORD_LENGTH_MAX && let Some(next_row) = next_row_size && let Some(next_row_ix) = next_row.1 && ix + word.len() >= next_row_ix {
          let orig_ix = &ix; 
          let mut ix = ix.to_owned();
          let mut peeker = row_sizes_iter.clone();
          let subword : String = word.char_indices().map(|(_i, x)| x).take(next_row_ix - orig_ix).collect();
          ix += subword.chars().count();
          let mut words : Vec<String> = [subword].to_vec();
          while let Some(next_row) = peeker.next() 
            && let Some(next_row_ix) = next_row.1 {
            if orig_ix + word.len() >= next_row_ix {
              let subword = word.char_indices().map(|(_i, x)| x).skip(ix - orig_ix).take(next_row_ix - ix).collect::<String>();
              ix += subword.chars().count();
              words.insert(words.len(), subword);
            }
          }
          if orig_ix + word.len() > ix {
            words.insert(words.len(), word.char_indices().map(|(_i, x)| x).skip(ix - orig_ix).collect());
          }
          words
        } else { 
          [word.char_indices().map(|(_i, x)| x).collect()].to_vec() 
        };
        
      for word in subwords {
        if let Some(next_row) = next_row_size && let Some(next_row_ix) = next_row.1 {
          if ix >= next_row_ix {
            should_include_row = row_include_iter.next();
            if let Some(include_row) = should_include_row && *include_row {
              if row_shown { ui.end_row(); ui.set_row_height(next_row.0); }
              ui.image(tex, emath::Vec2 { x: 1.0, y: next_row.0 });
              row_shown = true;
            }
            next_row_size = row_sizes_iter.next();
          }
        }
        ix += word.chars().count();

        if let Some(include_row) = should_include_row && *include_row {
          let emote = emotes.get(&word);
          if let Some(EmoteFrame { id, name: _, texture, path, zero_width }) = emote
              && let Some(texture) = texture.as_ref().or(emote_loader.transparent_img.as_ref()) {
            let (x, y) = (texture.size_vec2().x * (EMOTE_HEIGHT / texture.size_vec2().y), EMOTE_HEIGHT);
            if *zero_width {
              let (x, y) = last_emote_width.unwrap_or((x, y));
              let img = egui::Image::new(texture, egui::vec2(x, y));
              let cursor = ui.cursor().to_owned();
              let rect = egui::epaint::Rect { min: Pos2 {x: cursor.left() - x - ui.spacing().item_spacing.x, y: cursor.top()}, max:  Pos2 {x: cursor.left() - ui.spacing().item_spacing.x, y: cursor.bottom()} };
              img.paint_at(ui, rect);
            }
            else {
              ui.image(texture, egui::vec2(x, y)).on_hover_ui(|ui| {
                ui.label(format!("{}\n{}", word, path.replace("generated/", "").replace("/","")));
                ui.image(texture, texture.size_vec2());
              });
              last_emote_width = Some((x, y));
            }
          }
          else {
            last_emote_width = None;
            match &link_url {
              Some(url) => {
                let link = ui.add(egui::Label::new(RichText::new(word).size(BODY_TEXT_SIZE).color(ui.visuals().hyperlink_color)).sense(egui::Sense::click()));
                if link.hovered() {
                  ui.ctx().output().cursor_icon = egui::CursorIcon::PointingHand;
                }
                if link.clicked() {
                  let modifiers = ui.ctx().input().modifiers;
                  ui.ctx().output().open_url = Some(egui::output::OpenUrl {
                    url: url.clone(),
                    new_tab: modifiers.any(),
                  });
                }
                link
              },
              None => ui.label(RichText::new(word).size(BODY_TEXT_SIZE))
            };
          }
        }
      }
    }
  });
  ui_row.response.rect
}

fn is_url(word: &str) -> bool {
    //TODO: regex?
    word.starts_with("http")
}

pub fn get_chat_msg_header_layoutjob(for_display: bool, ui: &mut egui::Ui, channel_name: &str, channel_color: Color32, username: &String, timestamp: &DateTime<Utc>, profile: &UserProfile, _badges : Option<&HashMap<String, EmoteFrame>>) -> LayoutJob {
  let mut job = LayoutJob {
    wrap: TextWrapping { 
      break_anywhere: false,
      //max_width: ui.available_width() - ui.spacing().item_spacing.x - 1.,
      ..Default::default()
    },
    first_row_min_height: ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT),
    ..Default::default()
  };
  job.append(&format!("#{channel_name}"), 0., egui::TextFormat { 
    font_id: FontId::new(SMALL_TEXT_SIZE, FontFamily::Proportional), 
    color: channel_color.linear_multiply(0.6), 
    valign: Align::Center,
    ..Default::default()
  });
  job.append(&format!("[{}]", timestamp.format("%H:%M")), 3.0, egui::TextFormat { 
    font_id: FontId::new(SMALL_TEXT_SIZE, FontFamily::Proportional), 
    color: Color32::DARK_GRAY, 
    valign: Align::Center,
    ..Default::default()
  });
  if for_display { return job; }

  let badge_count = profile.badges.as_ref().and_then(|f| Some(f.len())).unwrap_or(0) as f32;
  let spacing = 3.0 + badge_count * (BADGE_HEIGHT + ui.spacing().item_spacing.x); // badges assumed to be square so height should equal width

  job.append(&format!("{}:", &profile.display_name.as_ref().unwrap_or(username)), spacing, egui::TextFormat {
    font_id: FontId::new(BODY_TEXT_SIZE, FontFamily::Proportional),
    color: convert_color(&profile.color),
    valign: Align::Center,
    ..Default::default()
  });
  job
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

  //println!("{} {} {}", rx, gx, bx);
  return Color32::from_rgb(rx, gx, bx);
}


pub struct EmoteFrame {
  pub id: String,
  pub name: String,
  pub path: String,
  //extension: Option<String>,
  pub texture: Option<egui::TextureHandle>,
  pub zero_width: bool
}

pub fn get_texture<'a> (emote_loader: &mut EmoteLoader, emote : &'a mut Emote, request : EmoteRequest) -> EmoteFrame {
  match emote.loaded {
    EmoteStatus::NotLoaded => {
      if let Err(e) = emote_loader.tx.try_send(request) {
        println!("Error sending emote load request: {}", e);
      }
      emote.loaded = EmoteStatus::Loading;
      EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width }
    },
    EmoteStatus::Loading => EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width},
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
                return EmoteFrame { texture: Some(frame.to_owned()), id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), zero_width: emote.zero_width };
              }
            }
            EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width }
          }
          else {
            let (frame, _delay) = frames.get(0).unwrap();
            EmoteFrame { texture: Some(frame.to_owned()), id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), zero_width: emote.zero_width }
          }
        },
        None => EmoteFrame { id: emote.id.to_owned(), name: emote.name.to_owned(), path: emote.path.to_owned(), texture: None, zero_width: emote.zero_width }
      }
    }
  }
}

fn get_provider_color(provider : &ProviderName) -> Color32 {
  match provider {
    //ProviderName::Twitch => Color32::from_rgba_unmultiplied(145, 71, 255, 255),
    ProviderName::Twitch => Color32::from_rgba_unmultiplied(169, 112, 255, 255),
    ProviderName::YouTube => Color32::from_rgba_unmultiplied(255, 78, 69, 255),
    _ => Color32::default()
  }
}