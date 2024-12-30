/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use chrono::{DateTime, Utc};
use egui::load::SizedTexture;
use egui::{Align2, ImageSource, Rounding, TextStyle, TextureHandle, Vec2};
use egui::{Color32, FontFamily, Align, RichText, text::LayoutJob, Pos2};
use itertools::Itertools;
use tracing::warn;

use crate::provider::ChatMessage;
use crate::{emotes::*, provider::{ProviderName, MessageType}};

use super::addtl_functions::{convert_color, get_body_text_style, get_text_style};
use super::UiChatMessage;

use super::consts::*;

pub fn display_combo_message(ui: &mut egui::Ui, row: &UiChatMessage, interactable: bool, emote_loader: &mut EmoteLoader) -> f32 {
  
  let ui_row = ui.horizontal(|ui| {
    ui.spacing_mut().interact_size.y = COMBO_LINE_HEIGHT;
    ui.set_min_height(COMBO_LINE_HEIGHT);
    
    //if let Some(combo) = row.combo.as_ref().and_then(|c| if c.is_final { Some(c) } else { None }) &&
    if let Some(combo) = row.message.combo_data.as_ref() {
        let emote = row.emotes.get(&combo.word);

        ui.horizontal(|ui| {
            // if let Some(transparent_img) = emote_loader.transparent_img.as_ref() {
            //     ui.image(ImageSource::Texture(SizedTexture::new(transparent_img.id(), emath::Vec2 { x: 1.0, y: COMBO_LINE_HEIGHT * 0.9 }))); // egui >= 0.23
            //     //ui.image(transparent_img.texture_id(ui.ctx()), emath::Vec2 { x: 1.0, y: COMBO_LINE_HEIGHT }); // egui <=0.21
            // }

            let job = get_chat_msg_header_layoutjob(false, ui, row.channel_display_info(), None, row.timestamp());
            ui.add(egui::Label::new(job).sense(egui::Sense { click: true, drag: false, focusable: false }));

            //let mut used_width : f32 = 0.;
            if let Some(emote) = emote && let Some(texture) = emote.get_texture(emote_loader, ui.ctx()) {
                add_ui_emote_image(&combo.word, &emote.path, texture, &emote.zero_width, &mut None, ui, COMBO_LINE_HEIGHT * 0.9, interactable);
                //used_width += r.map(|f| f.rect.width()).unwrap_or(0.);
            }

            let lbl = ui.add(egui::Label::new(RichText::new(format!(" x{} ", combo.count)).size(COMBO_LINE_HEIGHT * 0.75).italics()).sense(egui::Sense { click: true, drag: false, focusable: false }));
            let mut font = get_body_text_style(ui.ctx());
            font.size = COMBO_LINE_HEIGHT * 0.25;
            ui.painter().text(lbl.rect.center_top() + Vec2::new(0., -3.), Align2::CENTER_TOP, "COMBO", font, Color32::GRAY);
        });

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            ui.set_row_height(COMBO_LINE_HEIGHT / 3.);

            //let users = combo.users.iter().map(|x| &x.0).unique().join(" ");
            let size = COMBO_LINE_HEIGHT * 0.25;
            //let mut font = get_body_text_style(ui.ctx());
            //font.size = size;
            //let job = chat_estimate::get_text_rect_job(ui.available_width() - 10., users.as_str(), &0., font, false);
            
            //let rows = &ui.fonts(|f| f.layout_job(job)).rows;
            //if !rows.is_empty() {
                //let user_count = rows.iter().take(2).map(|x| x.text().split(' ').count()).sum();
                for (user, color) in combo.users.iter().unique()/*.take(user_count)*/ {
                    ui.add(egui::Label::new(RichText::new(user).size(size).color(*color)));
                }
    
                //ui.add(egui::Label::new(RichText::new(rows[0]..text()).size(COMBO_LINE_HEIGHT * 0.3).color(Color32::DARK_GRAY)));
            //}
        });
        
    }
  });

  if ui_row.response.rect.height() > COMBO_LINE_HEIGHT {
    warn!("{} {}", ui_row.response.rect.height(), COMBO_LINE_HEIGHT);
  }

  ui_row.response.rect.height()
}

pub fn display_chat_message(ui: &mut egui::Ui, chat_msg: &UiChatMessage, highlight: Option<Color32>, interactable: bool, emote_loader: &mut EmoteLoader) -> (f32, Option<String>, bool) {

  let emote_height = ui.text_style_height(&TextStyle::Body) * EMOTE_SCALING;
  let mut user_selected : Option<String> = None;
  let mut message_color : (u8,u8,u8) = (210,210,210);
  if chat_msg.message.is_removed.is_some() {
    message_color =  (180, 180, 180);
  }
  else if chat_msg.message.provider == ProviderName::DGG && chat_msg.message.message.starts_with('>') {
    message_color =  (99, 151, 37);
  }

  let mut msg_right_clicked = false;
  //let ui_row = ui.vertical(|ui| {
    let mut row_ix = 0;
    /*if chat_msg.is_ascii_art {
      ui.spacing_mut().item_spacing.y = 0.;
    }*/

    // let chat_msg_rows = chat_msg.row_data.iter().map(|row| {
    //   match &row.msg_char_range {
    //     TextRange::Range { range } => (chat_msg.message.message.char_indices().map(|(_i, x)| x).skip(range.start).take(range.end - range.start).collect::<String>(), row.is_visible, row.row_height, row.is_ascii_art),
    //     TextRange::EndRange { range } => (chat_msg.message.message.char_indices().map(|(_i, x)| x).skip(range.start).collect::<String>(), row.is_visible, row.row_height, row.is_ascii_art)
    //   }
    // });
    let chat_msg_rows : Vec<(&String, bool, f32, bool)> = vec![(&chat_msg.message.message, true, 0., false)];

    let mut height = 0.;

    for (row_text, is_visible, _row_height, is_ascii_art) in chat_msg_rows {

      let mut last_emote_width : Option<(f32, f32)> = None;
      if is_visible {
        let resp = ui.horizontal_wrapped(|ui| {

        // if let Some(transparent_img) = emote_loader.transparent_img.as_ref() {
        //   ui.image(ImageSource::Texture(SizedTexture::new(transparent_img.id(), emath::Vec2 { x: 1.0, y: row_height }))); // egui >= 0.23
        //   //ui.image(transparent_img.texture_id(ui.ctx()), emath::Vec2 { x: 1.0, y: row_height }); // egui <= 0.21
        // }
        //ui.set_row_height(emote_height);

        if let Some(highlight) = highlight {
          highlight_ui_row(ui, highlight);
        }

        if row_ix == 0 {
          let username = determine_name_to_display(chat_msg.message);
          let job = get_chat_msg_header_layoutjob(true, ui, chat_msg.channel_display_info(), chat_msg.username_display(), chat_msg.timestamp());
          ui.add(egui::Label::new(job).sense(egui::Sense::hover()));
          if let Some(user_badges) = &chat_msg.badges {
            for emote in user_badges {
              //let emote = chat_msg.badges.as_ref().and_then(|f| f.get(badge));
              if let Some(tex) = emote.get_texture(emote_loader, ui.ctx()) {
                let resp = ui.image(ImageSource::Texture(SizedTexture::new(tex.id(), egui::vec2(tex.size_vec2().x * (BADGE_HEIGHT / tex.size_vec2().y), BADGE_HEIGHT))));
                if interactable {
                  resp.on_hover_ui(|ui| {
                  //ui.image(tex.texture_id(ui.ctx()), egui::vec2(tex.size_vec2().x * (BADGE_HEIGHT / tex.size_vec2().y), BADGE_HEIGHT)).on_hover_ui(|ui| {
                    //ui.set_width(BADGE_HEIGHT + 20.);
                    //ui.vertical_centered(|ui| {
                      match chat_msg.message.provider {
                        ProviderName::Twitch => {
                          let parts = emote.name.split('/').collect_tuple::<(&str, &str)>().unwrap_or(("",""));
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
                        ProviderName::DGG => { ui.label(emote.display_name.as_ref().unwrap_or(&emote.name.to_owned())); },
                        ProviderName::YouTube => {}
                      };
  
                      ui.image(ImageSource::Texture(SizedTexture::new(tex.id(), tex.size_vec2())));
                    //});
                  });
                }
              }
            }
          }
    
          if let Some(uname_text) = username {
            let uname_rich_text = RichText::new(format!("{uname_text}:"))
              .font(get_body_text_style(ui.ctx()))
              .color(convert_color(chat_msg.user_color.as_ref().unwrap_or(&DEFAULT_USER_COLOR)));
            let uname = ui.add(egui::Label::new(uname_rich_text).sense(egui::Sense::click()));
            if interactable && uname.clicked() {
              user_selected = Some(uname_text.to_lowercase());
            }
            else if interactable && uname.secondary_clicked() {
              msg_right_clicked = true;
            }
            if interactable && uname.hovered() {
              ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
          }
        }

        let mut italicize = false;
        for word in row_text.split(' ') {
          if word == "ACTION" {
            italicize = true;
            continue;
          }

          let link_url = chat_msg.message.message.split_ascii_whitespace().find_or_first(|f| f.starts_with(word) || f.ends_with(word) || f.contains(word) && word.len() > 16).and_then(|f| if is_url(f) { Some(f) } else { None });
          let emote = chat_msg.emotes.get(word);
          /*if word == "👍" {
            // Can use a font rendering crate directly
            //   to output emoji chars as images scaled to emote size.
            // But waiting for one with rbg support (embedded svg/png)
            let font = include_bytes!("../../Roboto-Regular.ttf") as &[u8];
            let font = fontdue::Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();
            let (metrics, bitmap) = font.rasterize('👍', EMOTE_HEIGHT);
            let imgbufopt: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> =
              image::ImageBuffer::from_raw(metrics.width as u32, metrics.height as u32, bitmap);
            let image = image::DynamicImage::from(imgbufopt.unwrap_or_log());
            let tx = imaging::load_image_into_texture_handle(ui.ctx(), imaging::to_egui_image(image));
            let (x, y) = (tx.size_vec2().x * (EMOTE_HEIGHT / tx.size_vec2().y), EMOTE_HEIGHT);
            ui.image(&tx, egui::vec2(x, y));
          } else */ if let Some(emote) = emote {
            if let Some(tex) = emote.get_texture(emote_loader, ui.ctx()) {
              add_ui_emote_image(word, &emote.path, tex, &emote.zero_width, &mut last_emote_width, ui, emote_height, interactable);
            }
          }
          else {
            last_emote_width = None;
            match link_url {
              Some(url) => {
                let link = ui.add(egui::Label::new(RichText::new(word).font(get_body_text_style(ui.ctx())).color(ui.visuals().hyperlink_color)).sense(egui::Sense::click()).truncate());
                if link.hovered() {
                  ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
                if link.clicked() {
                  let modifiers = ui.ctx().input(|i| i.modifiers);

                  let url = if chat_msg.message.provider == ProviderName::DGG && let Some((prefix, suffix)) = url.split('/').collect_tuple() {
                    match prefix {
                      "#twitch" => format!("https://twitch.tv/{suffix}"),
                      "#youtube" => format!("https://www.youtube.com/watch?v={suffix}"),
                      _ => url.to_string()
                    }
                  } else {
                    url.to_string()
                  };

                  ui.ctx().output_mut(|o| o.open_url = Some(egui::output::OpenUrl {
                    url,
                    new_tab: modifiers.any(),
                  }));
                }
              },
              None => {
                let mut text = match is_ascii_art {
                  true => RichText::new(word).family(FontFamily::Monospace),
                  false => RichText::new(word).color(convert_color(&message_color))
                }.font(get_body_text_style(ui.ctx()));

                if italicize {
                  text = text.italics()
                }

                if let Some (mention) = chat_msg.mentions.as_ref().and_then(|f| f.iter().find(|m| word.to_lowercase().contains(&m.to_lowercase()))) {
                  let lbl = ui.add(egui::Label::new(mention.to_owned()).sense(egui::Sense::click()));
                  if interactable && lbl.clicked() {
                    user_selected = Some(mention.to_owned());
                  }
                  if interactable && lbl.hovered() {
                    ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                  }
                } else {
                  ui.add(egui::Label::new(text)/*.sense(egui::Sense::hover())*/);
                }
              }
            };
          }
        }
        // if let Some(img) = emote_loader.red_img.as_ref() {
        //     ui.image(ImageSource::Texture(SizedTexture::new(img.id(), emath::Vec2 { x: 1.0, y: emote_height })));
        // }
        //ui.end_row(); 
        });
        height += resp.response.rect.height();
      }
      row_ix += 1;
    }

  //});
  //let actual = format!("{:.2}", ui_row.response.rect.size().y + ui.spacing().item_spacing.y);
  //let expected = format!("{:.2}", chat_msg.row_data.iter().filter_map(|f| if f.is_visible { Some(f.row_height + ui.spacing().item_spacing.y) } else { None }).sum::<f32>());
  //if actual != expected {
    //info!("expected {} actual {} for {}", expected, actual, &chat_msg.message.username);
  //}
  //(ui_row.response.rect, user_selected, msg_right_clicked)

  if height == 0. { warn!("unexpected zero height result on chat message rendering"); }

  (height, user_selected, msg_right_clicked)
  //(egui::Rect::ZERO, user_selected, msg_right_clicked)
}

pub fn determine_name_to_display(chat_msg: &ChatMessage) -> Option<&String> {
  match &chat_msg.profile.display_name {
    _ if chat_msg.msg_type != MessageType::Chat => None,
    Some(display_name) if display_name.is_ascii() => Some(display_name),
    _ => Some(&chat_msg.username)
  }
}

fn add_ui_emote_image(word: &str, path: &str, texture: &TextureHandle, zero_width: &bool, last_emote_width: &mut Option<(f32, f32)>, ui: &mut egui::Ui, emote_height: f32, show_tooltip: bool) -> Option<egui::Response> {
  let (x, y) = (texture.size_vec2().x * (emote_height / texture.size_vec2().y), emote_height);
  if *zero_width {
    let (x, y) = last_emote_width.unwrap_or((x, y));
    let img = egui::Image::new(ImageSource::Texture(SizedTexture::new(texture.id(), egui::vec2(x, y))));
    let cursor = ui.cursor().to_owned();
    let rect = egui::epaint::Rect { min: Pos2 {x: cursor.left() - x - ui.spacing().item_spacing.x, y: cursor.top()}, max:  Pos2 {x: cursor.left() - ui.spacing().item_spacing.x, y: cursor.bottom()} };
    img.paint_at(ui, rect);
    None
  }
  else {
    let mut img = ui.image(ImageSource::Texture(SizedTexture::new(texture.id(), egui::vec2(x, y))));
    if show_tooltip {
      img = img.on_hover_ui(|ui| {
        ui.label(format!("{}\n{}", word, path.replace('/',"")));
        ui.image(ImageSource::Texture(SizedTexture::new(texture.id(), texture.size_vec2())));
      });
    }  
    *last_emote_width = Some((x, y));

    Some(img)
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

fn highlight_ui_row(ui: &egui::Ui, color: Color32) {
  let cursor = ui.cursor().to_owned();
  let rect = egui::epaint::Rect { 
    min: Pos2 {
      x: cursor.left() - 3., 
      y: cursor.top()}, 
    max:  Pos2 {
      x: cursor.left() + ui.available_width(), 
      y: cursor.bottom() + CHAT_ITEM_SPACING_Y }};
  ui.painter().rect_filled(
    rect, 
    Rounding::ZERO, 
    color
  );
}

fn is_url(word: &str) -> bool {
    //TODO: regex?
    word.starts_with("http") || word.starts_with("#twitch") || word.starts_with("#youtube")
}

pub fn get_chat_msg_header_layoutjob(
  for_display: bool, 
  ui: &egui::Ui, 
  channel_name: Option<(&str, Color32)>,
  user_name: Option<(&String, Color32)>,
  timestamp: Option<&DateTime<Utc>>
) -> LayoutJob {
  let mut job = LayoutJob {
    break_on_newline: false,
    first_row_min_height: ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT),
    ..Default::default()
  };
  if let Some((channel_name, channel_color)) = channel_name {
    job.append(&format!("#{channel_name} "), 0., egui::TextFormat { 
        font_id: get_text_style(TextStyle::Small, ui.ctx()), 
        color: channel_color.linear_multiply(0.6), 
        valign: Align::Center,
        ..Default::default()
      });
  }
  if let Some(timestamp) = timestamp {
    job.append(&format!("{} ", timestamp.with_timezone(&chrono::Local).format("%H:%M")), 0., egui::TextFormat { 
      font_id: get_text_style(TextStyle::Small, ui.ctx()),
      color: Color32::DARK_GRAY, 
      valign: Align::Center,
      ..Default::default()
    });
  }
  if for_display { return job; }

  if let Some((username, color)) = user_name {
    job.append(&format!("{}:", &username), ui.spacing().item_spacing.x, egui::TextFormat {
      font_id: get_body_text_style(ui.ctx()),
      color,
      valign: Align::Center,
      ..Default::default()
    });
  }
  job
}