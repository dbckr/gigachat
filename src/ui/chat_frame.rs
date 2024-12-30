use std::collections::HashMap;

use egui::Align;
use egui::Color32;
use egui::Pos2;
use egui::Rect;
use egui::RichText;
use egui::Rounding;
use egui::Stroke;
use egui::Vec2;
use tracing::error;
use tracing::warn;
use tracing_unwrap::OptionExt;

use crate::provider::channel::Channel;
use crate::provider::ChatMessage;
use crate::provider::MessageType;
use crate::provider::ProviderName;

use super::addtl_functions::*;
use super::chat;
use super::emote_selector::TextboxAndEmoteSelectorResponse;
use super::TemplateApp;
use super::consts::*;
use super::models::*;

impl TemplateApp {
    pub fn show_chat_frame(&mut self, id: &str, ui: &mut egui::Ui, mut chat_panel: ChatPanelOptions, ctx: &egui::Context, half_width: bool, popped_height: f32) -> ChatFrameResponse {
        
        //let mut msg_box_id : Option<egui::Id> = None;

        let mut response : ChatFrameResponse = Default::default();
        ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
            if half_width {
                ui.set_width(ui.available_width() / 2.);
            }
            
            let TextboxAndEmoteSelectorResponse { msg_box_id: _, selected_user_before } = self.render_textbox_and_emote_selector(ui, ctx, id, &mut chat_panel);

            //ui.painter().rect_filled(ui.available_rect_before_wrap(), Rounding::ZERO, Color32::LIGHT_RED);
            
            let mut chat_area = egui::ScrollArea::vertical()
            .id_salt(format!("chatscrollarea {id}"))
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .drag_to_scroll(chat_panel.selected_emote.is_none() && self.last_frame_ui_events.is_empty())
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);

            if let Some(chat_scroll) = chat_panel.chat_scroll {
                chat_area = chat_area.scroll_offset(egui::Vec2 {x: 0., y: chat_scroll.y - popped_height });
            }
            
            let mut overlay_viewport : Rect = Rect::NOTHING;
            let mut y_size = 0.;
            let mut y_size_new = 0.;
            let area = chat_area.show_viewport(ui, |ui, viewport| {  
                ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
                    overlay_viewport = viewport;
                    (y_size, y_size_new) = self.show_variable_height_rows(&mut chat_panel, ui, viewport);
                });
            });

            //TODO: y_size is ~8-15 less than the resulting area.content_size

            let max_scroll_offset = area.content_size.y - area.inner_rect.height();
            
            if chat_panel.chat_scroll_lock_to_bottom {
                // only increase scroll, never decrease (workaround for scrolling stutter on new messages)
                if chat_panel.chat_scroll.is_some_and(|f| max_scroll_offset > f.y) {
                    chat_panel.chat_scroll = Some(Vec2::new(0., max_scroll_offset));
                } else {
                    // do nothing if locked to bottom and new offset is less than cached
                }
            } else {
                chat_panel.chat_scroll = Some(area.state.offset);
            }

            let jump_rect = if area.content_size.y - (area.state.offset.y + y_size_new + area.inner_rect.height()) > 8. {
            //if (area.state.offset.y - (y_size - area.inner_rect.height())).abs() > 1. && y_size > area.inner_rect.height() {
            //if (area.content_size.y - overlay_viewport.max.y).abs() > 1. {
                let rect = Rect {
                    min: Pos2 { x: area.inner_rect.max.x - 60., y: area.inner_rect.max.y - 70. },
                    max: area.inner_rect.max
                };
                let jumpwin = egui::Window::new(format!("JumpToBottom {id}"))
                .fixed_rect(rect)
                .title_bar(false)
                .frame(egui::Frame { 
                    // inner_margin: egui::style::TextStyle::Margin::same(0.), 
                    // outer_margin: egui::style::Margin::same(0.),
                    rounding: Rounding::ZERO, 
                    shadow: eframe::epaint::Shadow::default(),
                    fill: Color32::TRANSPARENT,
                    stroke: Stroke::NONE,
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    if ui.button(RichText::new("🡳").size(48.).monospace()).clicked() {
                        chat_panel.chat_scroll_lock_to_bottom = true;
                        chat_panel.chat_scroll = Some(Vec2 { x: 0., y: max_scroll_offset });
                    } else {
                        chat_panel.chat_scroll_lock_to_bottom = false;
                    }
                });
                jumpwin.unwrap_or_log().response.rect
            } else { 
                Rect::NOTHING 
            };
            
            response.y_size = y_size;
            
            // Overlay for selected chatter's history
            //self.selected_user_chat_history_overlay(area.inner_rect, ui);
            // Window for selected chatter's history
            let history_rect = self.selected_user_chat_history_window(id, &mut chat_panel, ui.max_rect(), ctx);
            if history_rect != Rect::NOTHING && ctx.input(|i| i.pointer.any_click())
            && selected_user_before == chat_panel.selected_user
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            && area.inner_rect.contains(pos) 
            && !history_rect.contains(pos)
            && !jump_rect.contains(pos) {
                chat_panel.selected_user = None;
            }
        });
        response.state = chat_panel;
        //ui.memory_mut(|m| m.request_focus(msg_box_id.unwrap_or_log()));
        response
    }
    
    pub fn show_variable_height_rows(&mut self, chat_panel: &mut ChatPanelOptions, ui: &mut egui::Ui, viewport: Rect) -> (f32, f32) {
        let TemplateApp {
            chat_history_limit: _,
            body_text_size: _,
            bg_transparency: _,
            runtime : _,
            providers,
            channels,
            auth_tokens : _,
            hide_offline_chats : _,
            enable_combos,
            show_timestamps,
            show_muted,
            channel_tab_list : _,
            selected_channel : _,
            rhs_selected_channel : _,
            lhs_chat_state : _,
            rhs_chat_state : _,
            chat_histories,
            show_add_channel_menu : _,
            add_channel_menu : _,
            global_emotes,
            emote_loader,
            show_auth_ui : _,
            show_channel_options : _,
            twitch_chat_manager : _,
            show_timestamps_changed,
            dragged_channel_tab : _,
            rhs_tab_width: _,
            yt_chat_manager: _,
            enable_yt_integration: _,
            last_frame_ui_events: _,
            force_compact_emote_selector: _,
            discarded_last_frame: _
        } = self;
        
        let ChatPanelOptions {
            selected_channel,
            draft_message: _,
            chat_frame,
            chat_scroll: _,
            chat_scroll_lock_to_bottom: _,
            selected_user,
            selected_msg,
            selected_emote: _,
            selected_emote_input: _
        } = chat_panel;
        
        let mut y_pos = 0.0;
        let mut y_pos_before_visible = 0.0;
        let mut y_pos_visible = 0.0;
        let mut y_size_from_new_messages = 0.0;
        let mut set_selected_msg : Option<ChatMessage> = None;
            
        //let y_min = ui.max_rect().top() + viewport.min.y;
        //let y_max = ui.max_rect().top() + viewport.max.y;
        //let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
        //let mut in_view : Vec<UiChatMessage> = Default::default();
        let mut skipped_rows = 0;
        
        let mut _visible_rows: usize = 0;
        
        let mut history_iters = Vec::new();
        for (cname, hist) in chat_histories.iter_mut() {
            if selected_channel.as_ref().is_some_and(|channel| channel == cname) || selected_channel.is_none() && channels.get(cname).is_some_and(|f| f.shared().show_in_mentions_tab) {
                history_iters.push(hist.iter_mut().peekable());
            }
        }
        
        let mut history_iters = HistoryIterator {
            iterators: history_iters,
            //mentions_only: selected_channel.is_none(),
            //usernames: HashMap::default()// HashMap::from_iter(providers.iter().map(|(k, v)| (k.to_owned(), v.username.to_lowercase())))
        };
        let show_channel_names = history_iters.iterators.len() > 1;
        
        let mut usernames : HashMap<ProviderName, String> = HashMap::default();
        if selected_channel.is_none() {
            if let Some(twitch_chat_manager) = self.twitch_chat_manager.as_ref() {
                usernames.insert(ProviderName::Twitch, twitch_chat_manager.username.to_lowercase());
            }
            for (_, channel) in channels.iter_mut() {
                if let Channel::DGG { ref mut dgg, shared: _ } = channel && let Some(chat_mgr) = dgg.dgg_chat_manager.as_ref() {
                    usernames.insert(ProviderName::DGG, chat_mgr.username.to_lowercase()); 
                }
            }
        }
        
        let mut rows_drawn = 0;

        let viewport_size_drift = (chat_frame.map_or(Vec2::ZERO, |f| f.size()) - viewport.size()).length();

        if viewport_size_drift > 0. {
            warn!("viewport drift: {} {} {}", chat_frame.map_or(Vec2::ZERO, |f| f.size()), viewport.size(), chat_frame.map_or(Vec2::ZERO, |f| f.size()) - viewport.size());
        }

        let area_size_unchanged = chat_frame.is_some() && viewport_size_drift < 1.;
        if !area_size_unchanged {
            error!("viewport change: {} {} {}", chat_frame.map_or(Vec2::ZERO, |f| f.size()), viewport.size(), chat_frame.map_or(Vec2::ZERO, |f| f.size()) - viewport.size());
            ui.ctx().request_discard("ui resized");
        }

        *chat_frame = Some(viewport.to_owned());

        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().item_spacing.y = CHAT_ITEM_SPACING_Y;

        //let mut last_row;
        
        while let Some((row, cached_y)) = history_iters.get_next() {
            if selected_channel.is_none() && !mentioned_in_message(&usernames, &row.provider, &row.message) {
                continue;
            }
            
            // amount to render above and below the viewport area
            let overdraw_height = viewport.height() / 2.;

            // Skip processing if row size is accurately cached and not in view
            //TODO: also check if the font size or any other relevant setting has changed
            if !*show_timestamps_changed && area_size_unchanged 
            && let Some(size_y) = cached_y.as_ref().map(|y| match row.combo_data.as_ref() {
                None => y + CHAT_ITEM_SPACING_Y,
                Some(combo_data) => 
                    if !*enable_combos || combo_data.is_end && combo_data.count == 1 { y + CHAT_ITEM_SPACING_Y }
                    else if combo_data.is_end { 
                        //TODO: don't need this? since it will not be skipped if a cached_height has not been calc'd yet
                        combo_data.cached_height.unwrap_or(COMBO_LINE_HEIGHT + CHAT_ITEM_SPACING_Y)
                    } 
                    else { 0. }
            })
            &&  (size_y == 0. 
                    || y_pos < viewport.min.y - overdraw_height 
                    || y_pos + size_y > viewport.max.y + overdraw_height)
            &&  !row.combo_data.as_ref().is_some_and(|f| f.count > 1 && f.cached_height.is_none()) {

                //last_row = (row.to_owned(), cached_y.to_owned());

                y_pos += size_y;
                skipped_rows += 1;
                continue;
            }

            if rows_drawn == 0 {
                y_pos_before_visible = y_pos;

                // "draw" the empty space up to the start of the viewport area
                ui.add_space(y_pos_before_visible - CHAT_ITEM_SPACING_Y);

                // needed to maintain focus on any widget inside chat frame when scrolling (due to virtual/viewport rendering)
                ui.skip_ahead_auto_ids(skipped_rows);
                skipped_rows = 0;
            }
            
            let chat_msg = create_uichatmessage(row, show_channel_names, *show_timestamps, *show_muted, providers, channels, global_emotes);

            //TODO: remove this once viewport overflow check is added
            if cached_y.is_none() && (chat_msg.message.combo_data.is_none() || chat_msg.message.combo_data.as_ref().is_some_and(|f| f.is_new)) && !self.discarded_last_frame {
                //error!("new message -- frame discard");
                ui.ctx().request_discard("new chat msg");
                self.discarded_last_frame = true;
            }
            
            let rendered_height = if !*enable_combos || cached_y.is_none() || chat_msg.message.combo_data.is_none() || chat_msg.message.combo_data.as_ref().is_some_and(|c| c.count == 1 && c.is_end) {
                let highlight_msg = match chat_msg.message.msg_type {
                    MessageType::Announcement => Some(get_provider_color(&chat_msg.message.provider).linear_multiply(0.25)),
                    MessageType::Error => Some(Color32::from_rgba_unmultiplied(90, 0, 0, 90)),
                    MessageType::Information => Some(Color32::TRANSPARENT),
                    MessageType::Chat => if selected_user.as_ref() == Some(&chat_msg.message.profile.display_name.as_ref().unwrap_or(&chat_msg.message.username).to_lowercase()) {
                        Some(Color32::from_rgba_unmultiplied(90, 90, 90, 90))
                    } else {
                        None
                    }
                };
                let (height, user_selected, msg_right_clicked) = chat::display_chat_message(ui, &chat_msg, highlight_msg, chat_panel.selected_emote.is_none(), emote_loader);
                
                if user_selected.is_some() {
                    if *selected_user == user_selected {
                        *selected_user = None
                    } else {
                        *selected_user = user_selected
                    }
                }
                if msg_right_clicked {
                    set_selected_msg = Some(chat_msg.message.to_owned());
                }

                if cached_y.is_none() {
                    y_size_from_new_messages += height + CHAT_ITEM_SPACING_Y;
                }
                
                *cached_y = Some(height);

                height
            }
            else if chat_msg.message.combo_data.as_ref().is_some_and(|combo| combo.is_end) { 
                let height = chat::display_combo_message(ui, &chat_msg, chat_panel.selected_emote.is_none(), emote_loader);

                if let Some(combo) = row.combo_data.as_mut() {
                    // handle combo message height increasing as usernames are added
                    y_size_from_new_messages += match combo.cached_height {
                        Some(h) => height - h,
                        None => height + CHAT_ITEM_SPACING_Y
                    };

                    combo.cached_height = Some(height);
                }

                height
            } 
            else {
                // uncached, !is_end, >1 count emote combo message
                // do nothing

                continue;
            };

            y_pos += rendered_height + CHAT_ITEM_SPACING_Y;
            y_pos_visible += rendered_height + CHAT_ITEM_SPACING_Y;
            
            //ui.end_row();
            rows_drawn += 1;
        }

        //TODO: determine if viewport area overflowed -- if so, do not paint this frame

        // "draw" the empty space after the viewport area
        ui.add_space(y_pos - y_pos_before_visible - y_pos_visible - CHAT_ITEM_SPACING_Y);

        ui.skip_ahead_auto_ids(skipped_rows);
        
        if *show_timestamps_changed {
            *show_timestamps_changed = false;
        }
        
        set_selected_message(set_selected_msg, ui, selected_msg);
        
        (y_pos, y_size_from_new_messages)
    }
}