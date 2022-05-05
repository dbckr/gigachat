/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::HashMap;

use eframe::epaint::text::TextWrapping;
use egui::{Color32, text::LayoutJob, FontId, FontFamily};
use itertools::Itertools;

use crate::provider::*;

use super::{BODY_TEXT_SIZE, MIN_LINE_HEIGHT, EMOTE_HEIGHT, WORD_LENGTH_MAX, chat::{EmoteFrame, self}};

pub fn get_chat_msg_size(ui: &mut egui::Ui, row: &ChatMessage, emotes: &HashMap<String, EmoteFrame>, badges: Option<&HashMap<String, EmoteFrame>>) -> Vec<(f32, Option<usize>)> {
  // Use text jobs and emote size data to determine rows and overall height of the chat message when layed out
  let mut first_word_ix : Option<usize> = None;
  let mut curr_row_width : f32 = 0.0;
  let mut row_data : Vec<(f32, Option<usize>)> = Default::default();

  let job = chat::get_chat_msg_header_layoutjob(false, ui, &row.channel, Color32::WHITE, Some(&row.username), &row.timestamp, &row.profile, badges);
  let header_rows = &ui.fonts().layout_job(job.clone()).rows;
  for header_row in header_rows.iter().take(header_rows.len() - 1) {
    row_data.insert(row_data.len(), (header_row.rect.size().y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT), None));
  }
  curr_row_width += 1. + ui.spacing().item_spacing.x + header_rows.last().unwrap().rect.size().x + ui.spacing().item_spacing.x;
  let mut curr_row_height = header_rows.last().unwrap().rect.size().y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT);

  let mut ix = 0;
  for word in row.message.to_owned().split(" ") {
      get_word_size(&mut ix, emotes, word, ui, &mut curr_row_width, &mut curr_row_height, &mut row_data, &mut first_word_ix);
  }
  if curr_row_width > 0.0 {
    row_data.insert(row_data.len(), (curr_row_height.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT), first_word_ix));
  }
  row_data
}

fn get_word_size(ix: &mut usize, emotes: &HashMap<String, EmoteFrame>, word: &str, ui: &mut egui::Ui, curr_row_width: &mut f32, curr_row_height: &mut f32, row_data: &mut Vec<(f32, Option<usize>)>, first_word_ix: &mut Option<usize>) {
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
    get_text_rect(ui, word, &curr_row_width).into_iter().map(|row| (row.char_count_including_newline(), row.rect.size())).collect_vec()
  };
  for (char_len, row) in rows {    
    process_word_result(ui, row, curr_row_width, curr_row_height, row_data, first_word_ix, ix);
    *ix += char_len;
  }
}

fn process_word_result(ui: &mut egui::Ui, rect: egui::emath::Vec2, curr_row_width: &mut f32, curr_row_height: &mut f32, row_data: &mut Vec<(f32, Option<usize>)>, line_start_ix: &mut Option<usize>, ix: &mut usize) {
  let curr_width = *curr_row_width + rect.x + ui.spacing().item_spacing.x;
  let max_width = ui.available_width();
  if curr_width <= max_width {
    *curr_row_width += rect.x + ui.spacing().item_spacing.x;
    *curr_row_height = curr_row_height.max(rect.y);
  }
  else {
    row_data.insert(row_data.len(), (*curr_row_height, *line_start_ix));
    *curr_row_height = rect.y.max(ui.spacing().interact_size.y).max(MIN_LINE_HEIGHT);
    *curr_row_width = 1. + ui.spacing().item_spacing.x + rect.x + ui.spacing().item_spacing.x;
    *line_start_ix = Some(*ix);
  }
}

fn get_text_rect(ui: &mut egui::Ui, word: &str, curr_row_width: &f32) -> Vec<egui::epaint::text::Row> {
let mut job = LayoutJob {
  wrap: TextWrapping { 
    break_anywhere: if word.len() > WORD_LENGTH_MAX { true } else { false },
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