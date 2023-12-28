/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::HashMap, ops::{Range, RangeFrom}};
use egui::{Color32, text::LayoutJob, FontId, TextStyle};
use itertools::Itertools;
use crate::{ui::BADGE_HEIGHT, emotes::Emote};
use tracing_unwrap::OptionExt;

use crate::provider::*;

use super::{MIN_LINE_HEIGHT, WORD_LENGTH_MAX, chat::{self}, EMOTE_SCALING};

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

pub fn get_chat_msg_size(
  ui: &egui::Ui, 
  ui_width: f32, 
  row: &ChatMessage, 
  emotes: &HashMap<String, &Emote>, 
  badges: Option<&Vec<&Emote>>, 
  show_channel_name: bool, 
  show_timestamp: bool, 
  show_muted: bool
) -> Vec<(f32, TextRange, bool)> {
  // Use text jobs and emote size data to determine rows and overall height of the chat message when layed out
  let mut msg_char_range : TextRange = TextRange::Range { range: (0..0) };
  let mut curr_row_width : f32 = 0.0;
  let mut row_data : Vec<(f32, TextRange, bool)> = Default::default();
  let margin_width = 1. + ui.spacing().item_spacing.x; // single pixel image that starts each row
  //info!("ascii {}", is_ascii_art.is_some());

  let job = if show_channel_name {
    chat::get_chat_msg_header_layoutjob(false, ui, Some((&row.channel, Color32::WHITE)), chat::determine_name_to_display(row), &row.timestamp, &row.profile, show_timestamp)
  } else {
    chat::get_chat_msg_header_layoutjob(false, ui, None, chat::determine_name_to_display(row), &row.timestamp, &row.profile, show_timestamp)
  };

  let header_rows = &ui.fonts(|f| f.layout_job(job)).rows;
  for header_row in header_rows.iter().take(header_rows.len() - 1) {
    row_data.push((header_row.rect.size().y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT), TextRange::Range { range: (0..0) }, false));
  }
  let badge_count = badges.map(|f| f.len()).unwrap_or(0) as f32;
  let badge_spacing = badge_count * (BADGE_HEIGHT + ui.spacing().item_spacing.x); // badges assumed to be square so height should equal width
  let header_width = header_rows.last().unwrap_or_log().rect.size().x;
  curr_row_width += margin_width + header_width + ui.spacing().item_spacing.x + badge_spacing;
  let mut curr_row_height = header_rows.last().unwrap_or_log().rect.size().y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT);

  let mut ix = 0;
  let mut has_ascii_art: Option<usize> = None;
  let words = if !show_muted && let Some(removal_text) = row.is_removed.as_ref() { 
    removal_text.split_ascii_whitespace().collect_vec()
  } else { 
    row.message.split_ascii_whitespace().collect_vec()
  };
  for (i, word) in words.iter().enumerate() {
    has_ascii_art = match has_ascii_art {
      None => is_start_of_ascii_art(&words, i, emotes),
      Some(len) if word.len() == len => Some(len), // continuing ascii art
      _ => None // end of ascii art
    };
    if has_ascii_art.is_some() {
      row_data.push((curr_row_height, msg_char_range, true));
      curr_row_height = ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT);
      curr_row_width = margin_width;
      msg_char_range = TextRange::Range { range: (ix..ix) };
    }

    msg_char_range = get_word_size(ui, ui_width, &mut ix, emotes, word, &mut curr_row_width, &mut curr_row_height, &mut row_data, &msg_char_range, has_ascii_art);
    ix += 1;
  }
  if curr_row_width > margin_width {
    row_data.push((
      curr_row_height.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT), 
      //TextRange::Range { range: (msg_char_range.start()..ix - 1) } // -1 b/c last word may not have a trailing space
      msg_char_range,
      false
    ));
  }

  row_data
}

pub fn get_word_size(ui: &egui::Ui, ui_width: f32, ix: &mut usize, emotes: &HashMap<String, &Emote>, word: &str, 
  curr_row_width: &mut f32, curr_row_height: &mut f32, row_data: &mut Vec<(f32, TextRange, bool)>, curr_row_range: &TextRange, is_ascii_art: Option<usize>) -> TextRange
{
  let emote_height = ui.text_style_height(&TextStyle::Body) * EMOTE_SCALING;
  let mut row_start_char_ix = curr_row_range.start();

  if let Some(emote) = emotes.get(word) {
    let row = match emote.get_texture2() {
      _ if emote.zero_width => egui::vec2(0., 0.),
      Some(texture) => egui::vec2(texture.size_vec2().x * (emote_height / texture.size_vec2().y), emote_height),
      None => egui::vec2(emote_height, emote_height)
    };
    //process_row(&word.len(), &row, curr_row_width);
    let row_char_range = TextRange::Range { range: (row_start_char_ix..*ix) };
    if process_word_result(ui_width, &ui.spacing().item_spacing, &ui.spacing().interact_size, &row, curr_row_width, curr_row_height, row_data, row_char_range) {
      row_start_char_ix = *ix;
    }
    *ix += word.len();
  } else {
    let rows = get_text_rect(ui, ui_width, word, curr_row_width, is_ascii_art);
    let mut row_iter = rows.iter();
    while let Some((char_len, row)) = row_iter.next() {
      //process_row(char_len, row, curr_row_width);
      let row_char_range = TextRange::Range { range: (row_start_char_ix..*ix) };
      if process_word_result(ui_width, &ui.spacing().item_spacing, &ui.spacing().interact_size, row, curr_row_width, curr_row_height, row_data, row_char_range) {
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
  };
  
  // Return char range for the last row (which is not yet written to row_data)
  TextRange::Range { range: (row_start_char_ix..*ix) }
}

/// Returns true if starting a new row
fn process_word_result(
  available_width: f32, 
  item_spacing: &egui::Vec2, 
  interact_size: &egui::Vec2, 
  rect: &egui::Vec2, 
  curr_row_width: &mut f32, 
  curr_row_height: &mut f32, 
  row_data: &mut Vec<(f32, TextRange, bool)>, 
  row_char_range: TextRange
) -> bool {
  let curr_width = *curr_row_width + rect.x + item_spacing.x;
  if curr_width <= available_width {
    *curr_row_width += rect.x + item_spacing.x;
    *curr_row_height = curr_row_height.max(rect.y);
    false
  }
  else {
    row_data.push((*curr_row_height, row_char_range, false));
    *curr_row_height = rect.y.max(interact_size.y).max(MIN_LINE_HEIGHT);
    *curr_row_width = 1. + item_spacing.x + rect.x + item_spacing.x;
    true
  }
}

fn get_text_rect(ui: &egui::Ui, ui_width: f32, word: &str, curr_row_width: &f32, is_ascii_art: Option<usize>) -> Vec<(usize, egui::Vec2)> {
  let job = get_text_rect_job(ui_width - ui.spacing().item_spacing.x - 1., word, curr_row_width, crate::ui::get_body_text_style(ui.ctx()), is_ascii_art.is_some());
  let galley = ui.fonts(|f| f.layout_job(job));
  galley.rows.iter().map(|row| (row.char_count_including_newline(), row.rect.size())).collect_vec()
}

fn get_text_rect_job(max_width: f32, word: &str, width_used: &f32, font: FontId, is_ascii_art: bool) -> LayoutJob {
  let big_word = word.len() >= WORD_LENGTH_MAX || is_ascii_art;
  let mut job = LayoutJob {
    //wrap_width: max_width,
    //break_on_newline: word.len() >= WORD_LENGTH_MAX || is_ascii_art.is_some(),
    wrap: egui::epaint::text::TextWrapping { 
      break_anywhere: big_word,
      max_width: match big_word {
        true => max_width /*- 3.*/,
        false => max_width
      },
      ..Default::default()
    },
    ..Default::default()
  };

  job.append(word, width_used.to_owned(), egui::TextFormat { 
    font_id: font,
    ..Default::default() 
  });

  job
}

const ASCII_ART_MIN_LINES : usize = 5;
const ASCII_ART_MIN_LINE_WIDTH: usize = 15;

fn is_start_of_ascii_art(words: &[&str], ix: usize, emotes: &HashMap<String, &Emote>) -> Option<usize> {
  if words.len() - ix >= ASCII_ART_MIN_LINES && words[ix].len() > ASCII_ART_MIN_LINE_WIDTH && !emotes.contains_key(words[ix]) && words[ix..ix + ASCII_ART_MIN_LINES].iter().map(|w| w.len()).all_equal() {
    Some(words[ix].len())
  }
  else {
    None
  }
}