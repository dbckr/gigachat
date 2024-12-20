use std::ops::Add;

use egui::text::LayoutJob;
use egui::text_edit::TextEditState;
use egui::Color32;
use egui::Id;
use egui::Key;
use egui::Modifiers;
use egui::Rect;
use egui::Rounding;
use egui::TextStyle;
use egui::TextureHandle;
use egui::Vec2;
use tracing::info;
use tracing::warn;

use crate::provider::channel::Channel;
use crate::provider::ChatManagerRx;
use crate::provider::OutgoingMessage;

use super::TemplateApp;
use super::addtl_functions::*;
use super::consts::*;
use super::models::*;

impl TemplateApp {
    
    pub fn render_textbox_and_emote_selector(
        self: &mut Self, 
        ui: &mut egui::Ui, 
        ctx: &egui::Context, 
        id: &str,
        chat_panel: &mut ChatPanelOptions,
    ) -> TextboxAndEmoteSelectorResponse {

        let mut keep_focus_on_msg_box = false;

        let (goto_next_emote, goto_prev_emote, mut enter_emote) = if chat_panel.selected_emote_input.is_some() {
            let prev = ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowLeft)) || ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab));
            let next = ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowRight)) || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
            let enter_emote = chat_panel.selected_emote.as_ref().is_some_and(|x| !x.is_empty()) && (ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown)) /*|| input.consume_key(Modifiers::NONE, Key::Enter)*/);
            (next, prev, enter_emote)
        } 
        else { 
            (false, false, false) 
        };
        
        //ui.painter().rect_stroke(ui.max_rect(), Rounding::none(), Stroke::new(2.0, Color32::DARK_RED));
        let outgoing_msg_hint : egui::WidgetText = "Type a message to send".into();
        
        ui.style_mut().visuals.extreme_bg_color = Color32::from_rgba_premultiplied(0, 0, 0, 120);
        let mut draft_message = chat_panel.draft_message.to_owned();
        let mut outgoing_msg = egui::TextEdit::multiline(&mut draft_message)
        .desired_rows(2)
        .desired_width(ui.available_width())
        .hint_text(outgoing_msg_hint)
        .font(egui::TextStyle::Body)
        .lock_focus(chat_panel.selected_emote_input.is_some())
        .show(ui);
        
        let msg_box_id = Some(outgoing_msg.response.id);
        
        let update_ui_draft_msg = |word: &String, pos: &usize, emote_text: &String, draft_msg: &mut String, state: &mut TextEditState, finished: bool| {
            if !draft_msg.is_empty() {
                let end_pos = pos + word.len();
                let msg = if finished && (draft_msg.len() <= end_pos + 1 || &draft_msg[end_pos..end_pos + 1] != " ") {
                    format!("{}{} {}", &draft_msg[..*pos], emote_text, &draft_msg[end_pos..])
                } else {
                    format!("{}{}{}", &draft_msg[..*pos], emote_text, &draft_msg[end_pos..])
                };
                *draft_msg = msg;
                let cursor_pos = draft_msg[..*pos].len() + emote_text.len() + if finished { 1 } else { 0 };
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(cursor_pos))));
            }
        };
        
        for _ in 0..self.last_frame_ui_events.len() {
            match self.last_frame_ui_events.pop_front() {
                Some(UiEvent::EmoteSelectionEntered(frames_delay)) => {
                    if frames_delay == 0 {
                        outgoing_msg.response.request_focus()
                    } else {
                        self.last_frame_ui_events.push_back(UiEvent::EmoteSelectionEntered(frames_delay - 1))
                    }
                    
                },
                Some(event) => self.last_frame_ui_events.push_back(event),
                None => warn!("unexpected failure to pop last_frame_ui_events")
            }
        }
        
        let prev_history = outgoing_msg.response.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
        let next_history = outgoing_msg.response.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
        
        if (prev_history || next_history) && let Some(sc) = chat_panel.selected_channel.as_ref() && let Some(sco) = self.channels.get_mut(sc) {
            let mut ix = sco.shared().send_history_ix.unwrap_or(0);
            let msg = sco.shared().send_history.get(ix);
            if prev_history {
                ix = ix.add(1).min(sco.shared().send_history.len() - 1);
            } else {
                ix = ix.saturating_sub(1);
            };
            if let Some(msg) = msg {
                draft_message = msg.to_owned();
                outgoing_msg.state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(draft_message.len()))));
            }
            sco.shared_mut().send_history_ix = Some(ix);
        }
        
        if outgoing_msg.response.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) && !draft_message.is_empty() {
            if let Some(sc) = chat_panel.selected_channel.as_ref() && let Some(sco) = self.channels.get_mut(sc) {
                let (chat_tx, shared) = match sco {
                    Channel::Twitch { twitch: _, ref mut shared } => (self.twitch_chat_manager.as_mut().map(|m| m.in_tx()), shared),
                    Channel::DGG { ref mut dgg, ref mut shared } => (dgg.dgg_chat_manager.as_mut().map(|m| m.in_tx()), shared),
                    Channel::Youtube { youtube: _, ref mut shared } => (self.yt_chat_manager.as_mut().map(|m| m.in_tx()), shared)
                };
                if let Some(chat_tx) = chat_tx {
                    match chat_tx.try_send(OutgoingMessage::Chat { channel: shared.channel_name.to_owned(), message: draft_message.replace('\n', " ") }) {
                        Err(e) => info!("Failed to send message: {}", e), //TODO: emit this into UI
                        _ => {
                            shared.send_history.push_front(draft_message.trim_end().to_owned());
                            draft_message = String::new();
                            shared.send_history_ix = None;
                            chat_panel.selected_emote = None;
                            chat_panel.selected_emote_input = None;
                        }
                    }
                }
            } 
        }
        else if (outgoing_msg.response.has_focus() || !self.last_frame_ui_events.is_empty()) && !draft_message.is_empty() && let Some(cursor_pos) = outgoing_msg.state.cursor.char_range() {
            let cursor = cursor_pos.primary.index;
            let msg = &draft_message.to_owned();
            let word : Option<(usize, &str)> = msg.split_whitespace()
            .map(move |s| (s.as_ptr() as usize - msg.as_ptr() as usize, s))
            .filter_map(|p| if p.0 <= cursor && cursor <= p.0 + p.1.len() { Some((p.0, p.1)) } else { None })
            .next();
            
            let word_input = word.map(|x| (x.0.to_owned(), x.1.to_owned()));
            
            if chat_panel.selected_emote_input.is_none() || chat_panel.selected_emote.is_none() {
                chat_panel.selected_emote_input = word_input.to_owned();
            }
            
            if let Some((pos, word)) = chat_panel.selected_emote_input.as_ref().or(word_input.as_ref()) {
                
                let force_compact = !self.force_compact_emote_selector;
                let is_user_list = word.starts_with('@');
                let emotes = if is_user_list { 
                    self.get_possible_users(chat_panel.selected_channel.as_ref(), Some(word)) 
                } else { 
                    self.get_possible_emotes(chat_panel.selected_channel.as_ref(), Some(word), ctx) 
                };
                
                if let Some(emotes) = emotes && !emotes.is_empty() && let Some(textbox_word) = word_input.as_ref().map(|(_, str)| str) {
                    
                    let msg_rect = outgoing_msg.response.rect.to_owned();
                    let ovl_height = (ui.available_height() - msg_rect.height()) / 2.;
                    let mut painter_rect = msg_rect
                    .expand2(egui::vec2(0., ovl_height))
                    .translate(egui::vec2(0., (msg_rect.height() + ovl_height + 8.) * -1.));
                    
                    let emote_height = ui.text_style_height(&TextStyle::Body) * EMOTE_SCALING;
                    
                    let mut format = if !force_compact && !is_user_list {
                        SelectorFormat::EmoteOnly
                    } else {
                        SelectorFormat::EmoteAndText
                    };
                    
                    let enlarge_by_on_hover = if is_user_list { 0. } else { 0.5 };
                    let emote_height = if is_user_list { get_body_text_style(ctx).size + 4. } else { emote_height };
                    let mut emote_options = get_emote_rects(ui, ctx, emote_height, &painter_rect.shrink(emote_height * enlarge_by_on_hover / 2. + 10.), &emotes, &format);
                    if emotes.len() > emote_options.len() && format == SelectorFormat::EmoteAndText {
                        let alt_format = if !is_user_list { SelectorFormat::EmoteOnly } else { SelectorFormat::TextOnly };
                        if format != alt_format {
                            format = alt_format;
                            emote_options = get_emote_rects(ui, ctx, emote_height, &painter_rect.shrink(emote_height * enlarge_by_on_hover / 2. + 10.), &emotes, &format);
                        }
                    }
                    let drawn_emote_count = emote_options.len();
                    
                    let selector_height = if let Some(last_emote) = emote_options.iter().last() {
                        last_emote.0.top() - emote_height * enlarge_by_on_hover
                    } else { painter_rect.top() };
                    painter_rect.set_top(selector_height);
                    
                    egui::Window::new(format!("EmoteSelector {id}"))
                    .fixed_rect(painter_rect)
                    .title_bar(false)
                    //.anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(8., msg_rect.height() + 8.)) // glitchy
                    .frame(egui::Frame {
                        rounding: Rounding::ZERO, 
                        shadow: eframe::epaint::Shadow::NONE,
                        fill: Color32::from_rgba_unmultiplied(20, 20, 20, 200),
                        stroke: egui::Stroke::new(1., Color32::DARK_GRAY),
                        ..Default::default()
                    }.outer_margin(0.))
                    .show(ctx, |ui| { ui.horizontal(|ui| {
                        ui.expand_to_include_rect(painter_rect);
                        let painter = ui.painter_at(painter_rect);
                        //painter.set_layer_id(egui::LayerId::new(egui::Order::Debug, egui::Id::new(format!("emoteselector {id}"))));
                        
                        if ui.ui_contains_pointer() {
                            keep_focus_on_msg_box = true;
                            //outgoing_msg.response.request_focus();
                        }
                        
                        let mut selected_emote: Option<(egui::Rect, egui::Rect, &String, Option<&TextureHandle>)> = None;
                        let mut hovered_emote : Option<(egui::Rect, egui::Rect, &String, Option<&TextureHandle>)> = None;
                        
                        while let Some(emote_item) = emote_options.pop_front() {
                            let (emote_bg_rect, emote_img_rect, disp_text, texture) = emote_item;
                            
                            let hovered = ui.input(|i| i.pointer.hover_pos())
                            .map(|hover_pos| emote_bg_rect.contains(hover_pos))
                            .unwrap_or_default();
                            
                            if hovered {
                                hovered_emote = Some(emote_item);
                            }
                            
                            if chat_panel.selected_emote.is_none() && word == disp_text{
                                chat_panel.selected_emote = Some(disp_text.to_owned());
                            } 
                            
                            let emote_is_selected = chat_panel.selected_emote.as_ref() == Some(disp_text);
                            if emote_is_selected {
                                selected_emote = Some(emote_item);
                            }
                            
                            //painter.rect_filled(emote_bg_rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(20, 20, 20, 240));
                            
                            if let Some(texture) = texture {
                                let image = egui::Image::from_texture(texture)
                                .fit_to_exact_size(emote_img_rect.size())
                                .bg_fill(Color32::from_gray(20))
                                .tint(Color32::GRAY);
                                ui.put(emote_img_rect, image);
                            } else {
                                painter.rect_filled(emote_img_rect, Rounding::ZERO, Color32::DARK_GRAY);
                            }
                            
                            if format != SelectorFormat::EmoteOnly {
                                //ui.put(emote_img_rect, egui::Label::new(disp_text));
                                painter.text(
                                    emote_img_rect.left_center(),
                                    egui::Align2::LEFT_CENTER,
                                    disp_text,
                                    get_body_text_style(ctx),
                                    if emote_is_selected { Color32::LIGHT_GRAY } else { Color32::GRAY }
                                );
                            }
                            
                            if !hovered && emotes.len() > 1 && !emote_is_selected {
                                //painter.rect_filled(emote_bg_rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(20, 20, 20, 80));
                            }
                        }
                        
                        // draw outline around selected emote
                        // if let Some((_, _, hovered_disp_text, _)) = hovered_emote && let Some((emote_bg_rect, _, disp_text, _)) = selected_emote
                        //   && hovered_disp_text != disp_text {
                        //   painter.rect_stroke(emote_bg_rect, Rounding::ZERO, egui::Stroke::new(1., Color32::LIGHT_GRAY));
                        // }
                        
                        // draw larger version of hovered over emote
                        if format == SelectorFormat::EmoteOnly && let Some((_emote_bg_rect, emote_img_rect, disp_text, texture)) = hovered_emote.or(selected_emote) {
                            
                            if let Some(texture) = texture {
                                
                                let image = egui::Image::from_texture(texture)
                                .fit_to_exact_size(emote_img_rect.expand2(emote_img_rect.size() * enlarge_by_on_hover).size())
                                .bg_fill(Color32::from_gray(20)).sense(egui::Sense::click());
                                
                                let hovered = ui.put(emote_img_rect.expand2(emote_img_rect.size() * enlarge_by_on_hover), image);
                                
                                if hovered.clicked() {
                                    outgoing_msg.response.request_focus();
                                    chat_panel.selected_emote = Some(disp_text.to_owned());
                                    update_ui_draft_msg(textbox_word, pos, disp_text, &mut draft_message, &mut outgoing_msg.state, false);
                                    enter_emote = true;
                                }
                                
                                //if selected_emote.map(|e| e.2).is_some_and(|text| text == disp_text) {
                                if let Some((_size, text)) = chat_panel.selected_emote_input.as_ref() {
                                    let color = if text == disp_text { Color32::LIGHT_GRAY } else { Color32::DARK_GRAY };
                                    
                                    let enlarged_rect = emote_img_rect.expand2(emote_img_rect.size() * enlarge_by_on_hover);
                                    painter.rect_stroke(enlarged_rect, Rounding::ZERO, egui::Stroke::new(1., color));
                                }
                                
                                hovered.on_hover_text(egui::RichText::new(disp_text).color(Color32::WHITE));
                            }
                        }
                        
                        if emotes.len() > drawn_emote_count {
                            let disp_text = format!("and {} additional results...", emotes.len() - drawn_emote_count);
                            let mut job = LayoutJob {
                                wrap: egui::epaint::text::TextWrapping {
                                    break_anywhere: false,
                                    ..Default::default()
                                },
                                first_row_min_height: ui.spacing().interact_size.y.max(MIN_LINE_HEIGHT),
                                ..Default::default()
                            };
                            job.append(&disp_text, 0., egui::TextFormat {
                                font_id: get_body_text_style(ctx),
                                ..Default::default()
                            });
                            let galley = ui.fonts(|f| f.layout_job(job));
                            
                            let more_rect = egui::Rect {
                                min: painter_rect.min + Vec2::new(5., 5.),
                                max: painter_rect.min + Vec2::new(5., 5.) + galley.size()
                            };
                            
                            painter.rect_filled(more_rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(20, 20, 20, 240));
                            painter.text(
                                more_rect.left_center(),
                                egui::Align2::LEFT_CENTER,
                                disp_text,
                                get_body_text_style(ctx),
                                Color32::GRAY
                            );
                        }
                    })});
                    
                    if goto_next_emote {
                        if let Some(ix) = emotes.iter().position(|x| Some(&x.0) == chat_panel.selected_emote.as_ref()) && ix + 1 < emotes.len() {
                            chat_panel.selected_emote = emotes.get(ix + 1).map(|x| x.0.to_owned());
                        } else {
                            chat_panel.selected_emote = emotes.first().map(|x| x.0.to_owned());
                        }
                    }
                    else if goto_prev_emote && let Some(ix) = emotes.iter().position(|x| Some(&x.0) == chat_panel.selected_emote.as_ref()) && ix > 0 {
                        chat_panel.selected_emote = emotes.get(ix - 1).map(|x| x.0.to_owned());
                    }
                    else if chat_panel.selected_emote.is_some() && !emotes.iter().any(|x| Some(&x.0) == chat_panel.selected_emote.as_ref()) {
                        chat_panel.selected_emote = None;
                    }
                    else if chat_panel.selected_emote_input.is_some() && word_input.as_ref() != chat_panel.selected_emote_input.as_ref() && !emotes.iter().any(|x| Some(&x.0) == word_input.as_ref().map(|(_,str)| str)) {
                        chat_panel.selected_emote = None;
                    }
                    
                    if (goto_next_emote || goto_prev_emote) && let Some(emote_text) = &chat_panel.selected_emote && !emote_text.is_empty() {
                        update_ui_draft_msg(textbox_word, pos, emote_text, &mut draft_message, &mut outgoing_msg.state, false);
                    }
                    
                }
                else {
                    chat_panel.selected_emote = None;
                    chat_panel.selected_emote_input = None;
                }
            }
        }
        else {
            if let Some(emote_text) = chat_panel.selected_emote.as_ref()
            && let Some((pos, orig_input)) = chat_panel.selected_emote_input.as_ref() {
                update_ui_draft_msg(emote_text, pos, orig_input, &mut draft_message, &mut outgoing_msg.state, false);
            }
            
            chat_panel.selected_emote = None;
            chat_panel.selected_emote_input = None;
        }

        if keep_focus_on_msg_box {
            self.last_frame_ui_events.push_back(UiEvent::EmoteSelectionEntered(15));
        }
        
        ui.style_mut().visuals.override_text_color = Some(egui::Color32::LIGHT_GRAY);
        let selected_user_before = chat_panel.selected_user.as_ref().map(|x| x.to_owned());

        // Handle emote selection
        if enter_emote && let Some(emote_text) = chat_panel.selected_emote.as_ref() && !emote_text.is_empty()
        && let Some(pos) = chat_panel.selected_emote_input.as_ref().map(|i| &i.0) {
        update_ui_draft_msg(emote_text, pos, emote_text, &mut draft_message, &mut outgoing_msg.state, true);
        chat_panel.selected_emote = None;
        chat_panel.selected_emote_input = None;

        outgoing_msg.response.request_focus();
        }
        chat_panel.draft_message = draft_message;
        // needed for cursor reposition to take effect:
        outgoing_msg.state.store(ctx, outgoing_msg.response.id);

        TextboxAndEmoteSelectorResponse {
            msg_box_id,
            selected_user_before
        }
    }
} 

pub struct TextboxAndEmoteSelectorResponse {
    pub msg_box_id: Option<Id>,
    pub selected_user_before: Option<String>
}