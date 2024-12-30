use egui::Rect;
use egui::Vec2;
use itertools::Itertools;
use tracing_unwrap::OptionExt;

use crate::provider::ChatMessage;

use super::addtl_functions::*;
use super::chat;
use super::TemplateApp;
use super::models::*;

impl TemplateApp {
  pub fn selected_user_chat_history_window(&mut self, id: &str, chat_panel: &mut ChatPanelOptions, area: Rect, ctx: &egui::Context) -> Rect {
    let ChatPanelOptions {
      selected_channel,
      draft_message: _,
      chat_frame: _,
      chat_scroll: _,
      chat_scroll_lock_to_bottom: _,
      selected_user,
      selected_msg,
      selected_emote: _,
      selected_emote_input: _
    } = chat_panel;

    let rect = area.to_owned()
        .shrink2(Vec2 { x: area.width() / 7., y: area.height() / 4.})
        .translate(egui::vec2(area.width() / 9., area.height() * -0.25));
    if selected_user.is_some() && let Some(channel) = selected_channel.as_ref() {
      let window = egui::Window::new(format!("Selected User History {id}"))
      .fixed_rect(rect)
      .title_bar(false)
      .show(ctx, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let chat_area = egui::ScrollArea::vertical()
          .auto_shrink([false, true])
          .stick_to_bottom(true);
        chat_area.show_viewport(ui, |ui, _viewport| {  
          let mut msgs = self.chat_histories.get(channel).unwrap_or_log().iter().rev()
          .filter_map(|(msg, _)| if selected_user.as_ref() == Some(&msg.username) || selected_user.as_ref() == msg.profile.display_name.as_ref() { Some(msg.to_owned()) } else { None })
          .take(4)
          .collect_vec();
          if msgs.is_empty() {
            ui.label(format!("No recent messages for user: {}", selected_user.as_ref().unwrap_or_log()));
          } else {
            msgs.reverse();
            let mut set_selected_msg : Option<ChatMessage> = None;
            for msg in &msgs {
              //let transparent_texture = &mut self.emote_loader.transparent_img.as_ref().unwrap_or_log().to_owned();
              let est = create_uichatmessage(msg, false, self.show_timestamps, self.show_muted, &self.providers, &self.channels, &self.global_emotes);
              let (_, user_selected, msg_right_clicked) = chat::display_chat_message(ui, &est, None, chat_panel.selected_emote.is_none(), &mut self.emote_loader);
  
              if let Some(user) = user_selected.as_ref() {
                if selected_user.as_ref() == Some(user) {
                  *selected_user = None;
                } else {
                  *selected_user = Some(user.to_owned());
                }
              }
              if msg_right_clicked {
                set_selected_msg = Some(msg.to_owned());
              }
            }

            set_selected_message(set_selected_msg, ui, selected_msg);
          }
        });
      });

      window.unwrap_or_log().response.rect
    } else {
      Rect::NOTHING
    }
  }
}