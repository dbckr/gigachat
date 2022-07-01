/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::HashMap, ops::{Range, RangeFrom}};

use egui::epaint::text::TextWrapping;
use egui::{Color32, text::LayoutJob, FontId, FontFamily};
use itertools::Itertools;

use crate::provider::*;

use super::{BODY_TEXT_SIZE, MIN_LINE_HEIGHT, EMOTE_HEIGHT, WORD_LENGTH_MAX, chat::{EmoteFrame, self}};

#[derive(Debug)]
pub enum TextRange {
  Range { range: Range<usize> },
  EndRange { range: RangeFrom<usize> }
}

impl TextRange {
  pub fn start(&self) -> usize {
    match self {
      TextRange::Range { range } => range.start,
      TextRange::EndRange { range } => range.start
    }
  }
}

pub fn get_chat_msg_size(ui: &mut egui::Ui, row: &ChatMessage, emotes: &HashMap<String, EmoteFrame>, badges: Option<&HashMap<String, EmoteFrame>>, show_channel_names: bool) -> (Vec<(f32, TextRange)>, bool) {
  // Use text jobs and emote size data to determine rows and overall height of the chat message when layed out
  let mut msg_char_range : TextRange = TextRange::Range { range: (0..0) };
  let mut curr_row_width : f32 = 0.0;
  let mut row_data : Vec<(f32, TextRange)> = Default::default();
  let is_ascii_art = is_ascii_art(&row.message);
  //println!("ascii {}", is_ascii_art.is_some());

  let job = chat::get_chat_msg_header_layoutjob(false, ui, &row.channel, Color32::WHITE, Some(&row.username), &row.timestamp, &row.profile, show_channel_names);
  let header_rows = &ui.fonts().layout_job(job).rows;
  for header_row in header_rows.iter().take(header_rows.len() - 1) {
    row_data.insert(row_data.len(), (header_row.rect.size().y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT), TextRange::Range { range: (0..0) }));
  }
  curr_row_width += 1. + ui.spacing().item_spacing.x + header_rows.last().unwrap().rect.size().x + ui.spacing().item_spacing.x;
  let mut curr_row_height = header_rows.last().unwrap().rect.size().y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT);

  let mut ix = 0;
  for word in row.message.to_owned().split_ascii_whitespace() {
    if is_ascii_art.is_some() {
      row_data.insert(row_data.len(), (curr_row_height, msg_char_range));
      curr_row_height = ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT);
      curr_row_width = 1. + ui.spacing().item_spacing.x;
      msg_char_range = TextRange::Range { range: (ix..ix) };
    }

    msg_char_range = get_word_size(ui, &mut ix, emotes, word, &mut curr_row_width, &mut curr_row_height, &mut row_data, &msg_char_range, is_ascii_art);
    ix += 1;
  }
  if curr_row_width > 0.0 {
    row_data.insert(row_data.len(), (
      curr_row_height.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT), 
      //TextRange::Range { range: (msg_char_range.start()..ix - 1) } // -1 b/c last word may not have a trailing space
      msg_char_range
    ));
  }
  (row_data, is_ascii_art.is_some())
}

fn get_word_size(ui: &mut egui::Ui, ix: &mut usize, emotes: &HashMap<String, EmoteFrame>, word: &str, 
  curr_row_width: &mut f32, curr_row_height: &mut f32, row_data: &mut Vec<(f32, TextRange)>, curr_row_range: &TextRange, is_ascii_art: Option<usize>) -> TextRange
{
  let mut row_start_char_ix = curr_row_range.start();
  let rows : Vec<(usize, egui::emath::Vec2)> = if let Some(emote) = emotes.get(word) {
    if emote.zero_width {
      [(word.len(), egui::vec2(0., 0.))].to_vec()
    }
    else if let Some(texture) = emote.texture.as_ref() {
      [(word.len(), egui::vec2(texture.size_vec2().x * (EMOTE_HEIGHT / texture.size_vec2().y), EMOTE_HEIGHT))].to_vec()
    }
    else { // "standard" emote size until actual image is loaded
      [(word.len(), egui::vec2(EMOTE_HEIGHT, EMOTE_HEIGHT))].to_vec()
    }
  } else {
    get_text_rect(ui, word, curr_row_width, is_ascii_art).into_iter().map(|row| (row.char_count_including_newline(), row.rect.size())).collect_vec()
  };
  let mut row_iter = rows.iter();
  while let Some((char_len, row)) = row_iter.next() {
    let row_char_range = TextRange::Range { range: (row_start_char_ix..*ix) };
    let new_row = process_word_result(ui.available_width(), &ui.spacing().item_spacing, &ui.spacing().interact_size, row, curr_row_width, curr_row_height, row_data, row_char_range);
    if new_row {
      row_start_char_ix = *ix;
    }
    *ix += char_len;
    if is_ascii_art.is_some() {
      // truncate any overflow text instead of overflowing to more rows
      let range = TextRange::Range { range: (row_start_char_ix..*ix) };
      *ix += row_iter.map(|(char_len, _)| char_len).sum::<usize>();
      return range;
    }
  }
  // Return char range for the last row (which is not yet written to row_data)
  TextRange::Range { range: (row_start_char_ix..*ix) }
}

fn process_word_result(available_width: f32, item_spacing: &egui::Vec2, interact_size: &egui::Vec2, rect: &egui::Vec2, curr_row_width: &mut f32, curr_row_height: &mut f32, row_data: &mut Vec<(f32, TextRange)>, row_char_range: TextRange) -> bool {
  let curr_width = *curr_row_width + rect.x + item_spacing.x;
  if curr_width <= available_width {
    *curr_row_width += rect.x + item_spacing.x;
    *curr_row_height = curr_row_height.max(rect.y);
    false
  }
  else {
    row_data.insert(row_data.len(), (*curr_row_height, row_char_range));
    *curr_row_height = rect.y.max(interact_size.y).max(MIN_LINE_HEIGHT);
    *curr_row_width = 1. + item_spacing.x + rect.x +item_spacing.x;
    true
  }
}

fn get_text_rect(ui: &mut egui::Ui, word: &str, curr_row_width: &f32, is_ascii_art: Option<usize>) -> Vec<egui::epaint::text::Row> {
  let mut job = LayoutJob {
    wrap: TextWrapping { 
      break_anywhere: word.len() >= WORD_LENGTH_MAX || is_ascii_art.is_some(),
      max_width: ui.available_width() - ui.spacing().item_spacing.x - 1.,
      ..Default::default()
    },
    ..Default::default()
  };

  job.append(word, curr_row_width.to_owned(), egui::TextFormat { 
    font_id: FontId::new(BODY_TEXT_SIZE, FontFamily::Proportional), 
    ..Default::default() 
  });

  let galley = ui.fonts().layout_job(job);
  galley.rows.clone()
}

pub fn is_ascii_art(msg: &str) -> Option<usize> {
  let words = msg.split_ascii_whitespace().map(|w| w.len()).collect_vec();
  if words.len() > 1 && words.iter().all_equal() && let Some(len) = words.first() && len > &15 {
    Some(len.to_owned())
  }
  else {
    None
  }
}