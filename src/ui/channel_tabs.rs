use itertools::Itertools;
use tracing::debug;
use chrono::{DateTime, Utc};
use egui::text::LayoutJob;
use egui::{Align, Color32, Response, RichText, TextStyle, Ui};
use tracing_unwrap::OptionExt;

use crate::provider::channel::{Channel, ChannelTransient};
use crate::provider::dgg;

use super::TemplateApp;
use super::addtl_functions::*;
use super::models::*;

impl TemplateApp {

pub fn render_channel_tab_component(&mut self, ui: &mut Ui, ctx: &egui::Context) -> Option<UiEvent> {
    let mut removed_channel : Option<UiEvent> = None;

    ui.horizontal(|ui| {
        ui.horizontal_wrapped(|ui| {
          //let available_width = ui.available_width();
          if self.rhs_selected_channel.is_some() && let Some(width) = self.rhs_tab_width {
            ui.set_max_width(ui.available_width() - width);
          }

          let label = RichText::new("Mentions").text_style(TextStyle::Button);
          let clbl = ui.selectable_value(&mut self.selected_channel, None, label);
          if clbl.secondary_clicked() /*clbl.clicked_by(egui::PointerButton::Secondary)*/ {
            self.show_channel_options = Some((ctx.pointer_hover_pos().unwrap_or_log().to_vec2().to_owned(), "".to_owned()));
          }
    
          let mut tabs : Vec<(String, Response)> = Default::default();
          for channel in self.channel_tab_list.to_owned().iter() {
            if let Some(sco) = self.channels.get_mut(channel) && sco.transient().is_none() {
                debug!("Channel not opened yet, attempting to open: {}", channel);
                match sco {
                  Channel::Twitch { twitch, ref mut shared } => if let Some(chat_mgr) = self.twitch_chat_manager.as_mut() { 
                    //chat_mgr.open_channel(shared, Some(twitch.room_id.to_owned())); 
                    chat_mgr.open_channel(twitch, shared);
                  },
                  Channel::DGG { dgg, shared } => {
                    if let Some(chat_mgr) = dgg.dgg_chat_manager.as_mut() {
                      chat_mgr.close();
                    }
                    dgg.dgg_chat_manager = Some(dgg::open_channel(&self.auth_tokens.dgg_username, &self.auth_tokens.dgg_auth_token, dgg, shared, self.runtime.as_ref().unwrap_or_log(), &self.emote_loader, ctx));
                  },
                  Channel::Youtube { youtube: _, shared } => {
                    shared.transient = Some(ChannelTransient { 
                      channel_emotes: None,
                      badge_emotes: None,
                      status: None });
                  }
                }
              }

            let show_channel = self.rhs_selected_channel.as_ref() != Some(channel) && (
                !self.hide_offline_chats 
                || self.channels.get(channel).map(|c| c.shared().show_tab_when_offline || c.shared().transient.as_ref().and_then(|t| t.status.as_ref().map(|s| s.is_live)).unwrap_or(false)).unwrap_or(false)
            );
            if show_channel {
              let ChannelTabResponse { response, channel_removed } = self.ui_channel_tab(channel, ui, ctx);
              if let Some(removed) = channel_removed {
                removed_channel = Some(UiEvent::ChannelRemoved(removed));
              }
              else if let Some(clbl) = response {
                tabs.push((channel.to_owned(), clbl));
              }
            }
          }
    
            if let DragChannelTabState::DragStart(drag_channel, _) = &self.dragged_channel_tab {
                if let Some(ptr) = ctx.pointer_latest_pos() {
                    for ((l_channel, l_tab), (r_channel, r_tab)) in tabs.iter().tuple_windows() {
                        if ui.min_rect().contains(ptr) {
                            if l_channel == drag_channel && (ptr.x > r_tab.rect.left() && ptr.y > r_tab.rect.top() && ptr.y < r_tab.rect.bottom() || ptr.y > l_tab.rect.bottom()) {
                                let ix = self.channel_tab_list.iter().position(|x| x == l_channel);
                                if let Some(ix) = ix && ix < self.channel_tab_list.len() - 1 {
                                    self.channel_tab_list.swap(ix, ix + 1);
                                }
                            }
                            else if r_channel == drag_channel && (ptr.x < l_tab.rect.right() && ptr.y > l_tab.rect.top() && ptr.y < l_tab.rect.bottom() || ptr.y < r_tab.rect.top()) {
                                let ix = self.channel_tab_list.iter().position(|x| x == r_channel);
                                if let Some(ix) = ix && ix > 0 {
                                    self.channel_tab_list.swap(ix - 1, ix);
                                }
                            }
                        }
                    }
                }
            }
        });

        if let Some(channel) = self.rhs_selected_channel.to_owned() {
            let resp = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
              let inner_resp = self.ui_channel_tab(&channel, ui, ctx);

              if let Some(removed) = inner_resp.channel_removed {
                removed_channel = Some(UiEvent::ChannelRemoved(removed));
              }

              if ui.button("<").on_hover_text("Close split chat").clicked() {
                self.rhs_selected_channel = None;
              }
            });
            self.rhs_tab_width = Some(resp.response.rect.width());
        }
      });

    removed_channel
}

fn ui_channel_tab(&mut self, channel: &String, ui: &mut egui::Ui, ctx: &egui::Context) -> ChannelTabResponse {

    let mut channel_removed : Option<String> = None;

    if let Some(sco) = self.channels.get_mut(channel) {
      let provider = match sco {
        Channel::Twitch { twitch: _, shared: _ } => "Twitch",
        Channel::DGG { dgg: _, shared: _ } => "DGG Chat",
        Channel::Youtube { youtube: _, shared: _ } => "YouTube"
      };
      let shared = sco.shared_mut();
      if let Some(t) = shared.transient.as_ref() {            
        let mut job = LayoutJob { ..Default::default() };
        job.append(if channel.len() > 16 { &channel[0..15] } else { channel }, 0., egui::TextFormat {
          font_id: get_text_style(TextStyle::Button, ctx),
          color: Color32::LIGHT_GRAY,
          ..Default::default()
        });
        if channel.len() > 16 {
          job.append("..", 0., egui::TextFormat {
            font_id: get_text_style(TextStyle::Button, ctx),
            color: Color32::LIGHT_GRAY,
            ..Default::default()
          });
        }
        if t.status.as_ref().is_some_and(|s| s.is_live) {
          let red = if self.selected_channel.as_ref() == Some(&shared.channel_name) { 255 } else { 200 };
          job.append("🔴", 3., egui::TextFormat {
            font_id: get_text_style(TextStyle::Small, ctx),
            color: Color32::from_rgb(red, 0, 0),
            valign: Align::BOTTOM,
            ..Default::default()
          });
        }

        let clblx = crate::mod_selected_label::SelectableLabel::new(self.selected_channel == Some(channel.to_owned()), job);
        //let clblx = egui::SelectableLabel::new(self.selected_channel == Some(channel.to_owned()), job);
        let mut clbl = ui.add(clblx);
        
        if clbl.secondary_clicked() {
          self.show_channel_options = Some((ctx.pointer_hover_pos().unwrap_or_log().to_vec2().to_owned(), channel.to_owned()));
        }
        else if clbl.middle_clicked() {
          channel_removed = Some(channel.to_owned());
        }

        if clbl.drag_started_by(egui::PointerButton::Primary) && !matches!(&self.dragged_channel_tab, DragChannelTabState::DragStart(_, _)) {
          self.dragged_channel_tab = DragChannelTabState::DragStart(channel.to_owned(), self.channel_tab_list.to_owned());
        }
        else if clbl.drag_stopped() 
        && let DragChannelTabState::DragStart(_, drag_start_tab_order) = &self.dragged_channel_tab 
        && let Some(pos) = ctx.pointer_latest_pos() {
            let tab_order_changed = !&self.channel_tab_list.iter().eq(drag_start_tab_order);
            
            self.dragged_channel_tab = DragChannelTabState::DragRelease(channel.to_owned(), tab_order_changed, pos);
        }
        else if clbl.clicked_by(egui::PointerButton::Primary) {
            self.selected_channel = Some(channel.to_owned());
        }

        //if t.status.is_some_and(|s| s.is_live) || channel.len() > 16 {
          clbl = clbl.on_hover_ui(|ui| {
            if let Some(status) = &t.status && status.is_live {
              ui.label(RichText::new(format!("{channel} ({provider})")).size(get_body_text_style(ctx).size * 1.5));
              if let Some(title) = status.title.as_ref() {
                ui.label(title);
              }
              if let Some(game) = status.game_name.as_ref() {
                ui.label(game);
              }
              if let Some(viewers) = status.viewer_count.as_ref() {
                ui.label(format!("{viewers} viewers"));
              }
          
              if let Some(started_at) = status.started_at.as_ref() { 
                if let Ok(dt) = DateTime::parse_from_rfc3339(started_at) {
                  let dur = chrono::Utc::now().signed_duration_since::<Utc>(dt.to_utc()).num_seconds();
                  let width = 2;
                  ui.label(format!("Live for {:0width$}:{:0width$}:{:0width$}:{:0width$}", dur / 60 / 60 / 24, dur / 60 / 60 % 60, dur / 60 % 60, dur % 60));
                }
                else if let Ok(dt) = DateTime::parse_from_str(started_at, "%Y-%m-%dT%H:%M:%S%z") {
                  let dur = chrono::Utc::now().signed_duration_since::<Utc>(dt.to_utc()).num_seconds();
                  let width = 2;
                  ui.label(format!("Live for {:0width$}:{:0width$}:{:0width$}:{:0width$}", dur / 60 / 60 / 24, dur / 60 / 60 % 60, dur / 60 % 60, dur % 60));
                }
              }
            }
            else {
              ui.label(format!("{channel} ({provider})"));
            }
          });
        //}
        
        return ChannelTabResponse {
            response: Some(clbl),
            channel_removed
        };
      }
    }
    
    ChannelTabResponse {
        response: None,
        channel_removed
    }
  }
}

struct ChannelTabResponse {
    response: Option<Response>,
    channel_removed: Option<String>
}