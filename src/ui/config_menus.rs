use tracing::{error, warn};
use tracing_unwrap::{OptionExt, ResultExt};
use std::collections::HashMap;
use egui::{Color32, Key, OpenUrl, RichText, TextStyle};
use crate::provider::{dgg, twitch::{self, TwitchChatManager}, Provider, ProviderName};
use crate::provider::channel::{Channel, YoutubeChannel, ChannelShared};
use crate::emotes::EmoteRequest;


use super::models::*;

use super::TemplateApp;

impl TemplateApp {
    pub fn render_menubar(self: &mut Self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            egui::menu::bar(ui, |ui| {
                if ui.menu_button(RichText::new("Add a channel").text_style(TextStyle::Small), |ui| { ui.close_menu(); }).response.clicked() {
                    self.show_add_channel_menu = true;
                }
                ui.separator();
                if ui.menu_button(RichText::new("Configure Logins").text_style(TextStyle::Small), |ui| { ui.close_menu(); }).response.clicked() {
                    self.show_auth_ui = true;
                }
                ui.separator();
                ui.menu_button(RichText::new("Options").text_style(TextStyle::Small), |ui| {
                    ui.scope(|ui| {
                        let fontid = TextStyle::Button.resolve(ui.style().as_ref());
                        ui.style_mut().text_styles.insert(TextStyle::Body, fontid);
                        
                        ui.add(egui::Slider::new(&mut self.bg_transparency, 0..=255).step_by(1.).text(RichText::new("Background Transparency").text_style(TextStyle::Small)));
                        ui.add(egui::Slider::new(&mut self.body_text_size, 10.0..=40.0).step_by(0.5).text(RichText::new("Font Size").text_style(TextStyle::Small)));
                        ui.checkbox(&mut self.hide_offline_chats, "Hide Offline Chats").on_hover_text("Hide offline channel tabs. Can force specific channels to always show using channel level options menu.");
                        ui.checkbox(&mut self.enable_combos, "Enable Combos").on_hover_text("Multiple consecutive messages with the same emote will be reduced to a single line \"combo counter\".");
                        if ui.checkbox(&mut self.show_timestamps, "Show Message Timestamps").changed() {
                            self.show_timestamps_changed = true;
                        };
                        if ui.checkbox(&mut self.show_muted, "Show Muted/Banned Messages").changed() {
                            self.show_timestamps_changed = true;
                        };
                        ui.checkbox(&mut self.force_compact_emote_selector, "Force Compact Emote Selector").on_hover_text("Only show emote images in selector. If disabled, selector will show emote text alongside images, if all emotes can fit into displayable area.");
                        ui.checkbox(&mut self.enable_yt_integration, "Enable YT Integration");
                        ui.add(egui::Slider::new(&mut self.chat_history_limit, 100..=10000).step_by(100.).text(RichText::new("Chat history limit").text_style(TextStyle::Small)));
                        if ui.button("Reload Global and TTV Sub Emotes").clicked() {
                            if let Err(e) = self.emote_loader.tx.try_send(EmoteRequest::GlobalEmoteListRequest { force_redownload: true }) {
                                warn!("Failed to send request: {e}");
                            }
                            if let Err(e) = self.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { 
                                token: self.auth_tokens.twitch_auth_token.to_owned(), 
                                force_redownload: true 
                            }) {
                                warn!("Failed to send request: {e}");
                            }
                            let twitch_auth = &self.auth_tokens.twitch_auth_token;
                            if let Some(provider) = self.providers.get(&ProviderName::Twitch) {
                                for emote_set_id in &provider.my_emote_sets {
                                    if let Err(e) = self.emote_loader.tx.try_send(EmoteRequest::TwitchEmoteSetRequest { 
                                        token: twitch_auth.to_owned(), 
                                        emote_set_id: emote_set_id.to_owned(), 
                                        force_redownload: true }) 
                                        {
                                            warn!("Failed to send request: {e}");
                                        }
                                    }
                                }
                            }
                        });
                    });
                    ui.separator();
                    if ui.menu_button(RichText::new("View on Github").text_style(TextStyle::Small), |ui| { ui.close_menu(); }).response.clicked() {
                        ctx.open_url(OpenUrl::new_tab("https://github.com/dbckr/gigachat"))
                    }
                    ui.separator();
                    ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).text_style(TextStyle::Small).color(Color32::DARK_GRAY));
                    ui.separator();
                    
                    let tx_len = self.emote_loader.tx.len();
                    let rx_len = self.emote_loader.rx.len();
                    if cfg!(feature = "debug-ui") {
                        ui.label(RichText::new(format!("tx: {tx_len}, rx: {rx_len}")).text_style(TextStyle::Small).color(Color32::DARK_GRAY));
                    }
                });
            });
        }
        
    pub fn ui_auth_menu(&mut self, ctx: &egui::Context) {
        let mut changed_twitch_token = false;
        let mut changed_dgg_token = false;
        if self.show_auth_ui {
            let auth_menu = egui::Window::new("Auth Tokens").collapsible(false).show(ctx, |ui| {
                ui.scope(|ui| {
                    let fontid = TextStyle::Button.resolve(ui.style().as_ref());
                    ui.style_mut().text_styles.insert(TextStyle::Body, fontid);
                    
                    ui.horizontal(|ui| {
                        ui.label("Twitch Username:");
                        ui.text_edit_singleline(&mut self.auth_tokens.twitch_username);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Twitch Token:");
                        if self.auth_tokens.show_twitch_auth_token {
                            ui.text_edit_singleline(&mut self.auth_tokens.twitch_auth_token);
                        }
                        else if !self.auth_tokens.twitch_auth_token.is_empty() {
                            ui.label("<Auth token hidden>");
                        }
                        else {
                            ui.label("Not logged in");
                        }
                        if ui.button("Log In").clicked() {
                            self.auth_tokens.twitch_auth_token = "".to_owned();
                            self.auth_tokens.show_twitch_auth_token = true;
                            ctx.open_url(OpenUrl::new_tab(twitch::authenticate()));
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("DGG Username:");
                        ui.text_edit_singleline(&mut self.auth_tokens.dgg_username);
                    });
                    ui.horizontal(|ui| {
                        ui.label("DGG Token:");
                        if self.auth_tokens.show_dgg_auth_token {
                            ui.text_edit_singleline(&mut self.auth_tokens.dgg_auth_token);
                        }
                        else if !self.auth_tokens.dgg_auth_token.is_empty() {
                            ui.label("<Auth token hidden>");
                        }
                        else {
                            ui.label("Not logged in");
                        }
                        if ui.button("Log In").clicked() {
                            self.auth_tokens.dgg_auth_token = "".to_owned();
                            self.auth_tokens.show_dgg_auth_token = true;
                            ctx.open_url(OpenUrl::new_tab("https://www.destiny.gg/profile/developer"));
                            //self.auth_tokens.dgg_verifier = dgg::begin_authenticate(ctx);
                        }
                    });
                    if self.auth_tokens.show_dgg_auth_token {
                        ui.horizontal(|ui| {
                            ui.label("  go to destiny.gg > Account > Developers > Connections > Add login key");
                        });
                    }
                    /*ui.horizontal(|ui| {
                    ui.label("YouTube");
                    ui.text_edit_singleline(&mut self.auth_tokens.youtube_auth_token);
                    });
                    ui.separator();*/
                    if ui.button("Ok").clicked() {
                        changed_twitch_token = self.auth_tokens.show_twitch_auth_token;
                        changed_dgg_token = self.auth_tokens.show_dgg_auth_token;
                        let twitch_token = self.auth_tokens.twitch_auth_token.to_owned();
                        if twitch_token.starts_with('#') || twitch_token.starts_with("access") {
                            let rgx = regex::Regex::new("access_token=(.*?)&").unwrap_or_log();
                            let cleaned = rgx.captures(twitch_token.as_str()).unwrap_or_log().get(1).map_or("", |x| x.as_str());
                            self.auth_tokens.twitch_auth_token = cleaned.to_owned();
                            if !cleaned.is_empty() {
                                self.auth_tokens.show_twitch_auth_token = false;
                            }
                        }
                        let dgg_token = self.auth_tokens.dgg_auth_token.to_owned();
                        /*if dgg_token.starts_with('?') || dgg_token.starts_with("code") {
                        let rgx = regex::Regex::new("code=(.*?)&").unwrap_or_log();
                        let cleaned = rgx.captures(dgg_token.as_str()).unwrap_or_log().get(1).map_or("", |x| x.as_str());
                        if !cleaned.is_empty() {
                        let verifier = self.auth_tokens.dgg_verifier.to_owned();
                        let token = dgg::complete_authenticate(cleaned, &verifier).await;
                        
                        self.auth_tokens.dgg_auth_token = token.expect_or_log("failed to get dgg token");
                        self.auth_tokens.dgg_verifier = Default::default();
                        self.auth_tokens.show_dgg_auth_token = false;
                        }
                        }
                        else*/ if !dgg_token.is_empty() {
                            self.auth_tokens.show_dgg_auth_token = false;
                        }
                        self.show_auth_ui = false;
                    }
                });
            }).unwrap_or_log();
            if ctx.input(|i| i.pointer.any_click())
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            && !auth_menu.response.rect.contains(pos) {
                self.show_auth_ui = false;
            }
            else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.show_auth_ui = false;
            }
        }
        if changed_twitch_token {
            if let Some(mgr) = self.twitch_chat_manager.as_mut() {
                mgr.close();
            }
            if !self.auth_tokens.twitch_auth_token.is_empty() {
                let mut mgr = TwitchChatManager::new(&self.auth_tokens.twitch_username, &self.auth_tokens.twitch_auth_token, self.runtime.as_ref().unwrap_or_log(), ctx);
                for (_, channel) in self.channels.iter_mut() {
                    if let Channel::Twitch { twitch, ref mut shared } = channel {
                        //mgr.open_channel(shared, Some(twitch.room_id.to_owned()))
                        mgr.open_channel(twitch, shared);
                    }
                }
                self.twitch_chat_manager = Some(mgr);
                match self.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { token: self.auth_tokens.twitch_auth_token.to_owned(), force_redownload: false }) {  
                    Ok(_) => {},
                    Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
                };
            }
        }
        if changed_dgg_token {
            for (_, channel) in self.channels.iter_mut() {
                if let Channel::DGG { dgg, ref mut shared } = channel {
                    if let Some(chat_mgr) = dgg.dgg_chat_manager.as_mut() {
                        chat_mgr.close();
                    }
                    dgg.dgg_chat_manager = Some(dgg::open_channel(&self.auth_tokens.dgg_username, &self.auth_tokens.dgg_auth_token, dgg, shared, self.runtime.as_ref().unwrap_or_log(), &self.emote_loader, ctx));
                }
            }       
        }
    }
    
    pub fn ui_add_channel_menu(&mut self, ctx: &egui::Context) {
        let mut add_channel = |providers: &mut HashMap<ProviderName, Provider>, auth_tokens: &mut AuthTokens, channel_options: &mut AddChannelMenu| {
            let c = match channel_options.provider {
                ProviderName::Twitch => { 
                    providers.entry(ProviderName::Twitch).or_insert(Provider {
                        name: "twitch".to_owned(),
                        my_sub_emotes: Default::default(),
                        emotes: Default::default(),
                        global_badges: Default::default(),
                        username: Default::default(),
                        my_emote_sets: Default::default()
                    });
                    match self.emote_loader.tx.try_send(EmoteRequest::TwitchGlobalBadgeListRequest { token: auth_tokens.twitch_auth_token.to_owned(), force_redownload: false }) {  
                        Ok(_) => {},
                        Err(e) => { error!("Failed to request global emote json due to error {:?}", e); }
                    };
                    if self.twitch_chat_manager.is_none() {
                        self.twitch_chat_manager = Some(TwitchChatManager::new(&auth_tokens.twitch_username, &auth_tokens.twitch_auth_token, self.runtime.as_ref().unwrap_or_log(), ctx));
                    }
                    self.twitch_chat_manager.as_mut().unwrap_or_log().init_channel(&channel_options.channel_name)
                },
                ProviderName::DGG => dgg::init_channel(),
                ProviderName::YouTube => {
                    providers.entry(ProviderName::YouTube).or_insert(Provider {
                        name: "YouTube".to_owned(),
                        my_sub_emotes: Default::default(),
                        emotes: Default::default(),
                        global_badges: Default::default(),
                        username: Default::default(),
                        my_emote_sets: Default::default()
                    });
                    
                    Channel::Youtube { 
                        youtube: YoutubeChannel {}, 
                        shared: ChannelShared { ..Default::default() } 
                    }
                }
                /*ProviderName::YouTube => {
                if providers.contains_key(&ProviderName::Twitch) == false {
                providers.insert(ProviderName::Twitch, Provider {
                name: "youtube".to_owned(),
                my_sub_emotes: Default::default(),
                emotes: Default::default(),
                global_badges: Default::default()
                });
                }
                youtube::init_channel(channel_options.channel_name.to_owned(), channel_options.channel_id.to_owned(), auth_tokens.youtube_auth_token.to_owned(), self.runtime.as_ref().unwrap_or_log())
                }*/
            };
            
            let name = c.channel_name().to_owned();
            if self.channels.try_insert(name.to_owned(), c).is_ok() {
                self.channel_tab_list.push(name.to_owned());
            }
            self.selected_channel = Some(name);
            channel_options.channel_name = Default::default();
        };
        if self.show_add_channel_menu {
            let add_menu = egui::Window::new("Add Channel").collapsible(false).show(ctx, |ui| {
                ui.scope(|ui| {
                    let fontid = TextStyle::Button.resolve(ui.style().as_ref());
                    ui.style_mut().text_styles.insert(TextStyle::Body, fontid);
                    
                    let mut name_input : Option<egui::Response> = None;
                    ui.horizontal(|ui| {
                        ui.label("Provider:");
                        ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::Twitch, "Twitch");
                        //ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::YouTube, "Youtube");
                        ui.selectable_value(&mut self.add_channel_menu.provider, ProviderName::DGG, "destiny.gg");
                    });
                    if self.add_channel_menu.provider == ProviderName::Twitch {
                        ui.horizontal(|ui| {
                            ui.label("Channel Name:");
                            name_input = Some(ui.text_edit_singleline(&mut self.add_channel_menu.channel_name));
                            //name_input.request_focus();
                        });
                    }
                    /*if self.add_channel_menu.provider == ProviderName::YouTube {
                    ui.horizontal(|ui| {
                    ui.label("Channel ID:");
                    ui.text_edit_singleline(&mut self.add_channel_menu.channel_id);
                    });
                    }*/
                    
                    if name_input.is_some() && !self.add_channel_menu.channel_name.starts_with("YT:") && name_input.unwrap_or_log().has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.button("Add channel").clicked() {
                        add_channel(&mut self.providers, &mut self.auth_tokens, &mut self.add_channel_menu); 
                        self.show_add_channel_menu = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_add_channel_menu = false;
                    }
                });
            }).unwrap_or_log();
            if ctx.input(|i| i.pointer.any_click())
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            && !add_menu.response.rect.contains(pos) {
                self.show_add_channel_menu = false;
            }
            else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.show_add_channel_menu = false;
            }
        }
    } 

    pub fn ui_channel_options(&mut self, ctx: &egui::Context) -> Option<String> {
        let mut channel_removed : Option<String> = None;
        if self.show_channel_options.is_some() {
          let (pointer_vec, channel) = self.show_channel_options.to_owned().unwrap_or_log();
          let add_menu = egui::Window::new(format!("Configure Channel: {channel}"))
          .anchor(egui::Align2::LEFT_TOP, pointer_vec)
          .collapsible(false)
          .resizable(false)
          .show(ctx, |ui| {
            if !channel.is_empty() {
              if let Some(ch) = self.channels.get_mut(&channel) {
                let resp = ui.checkbox(&mut ch.shared_mut().show_tab_when_offline, "Always Show Tab").on_hover_text("Ignore the Hide Offline setting and always display this channel in tab list.");
                if resp.changed() && let Some(mgr) = self.twitch_chat_manager.as_mut() && let Channel::Twitch { twitch, shared } = ch {
                    mgr.open_channel(twitch, shared);
                }
              }
              ui.separator();
              if ui.button("Remove channel").clicked() {
                channel_removed = Some(channel.to_owned());
                self.show_channel_options = None;
              }
              if ui.button("Reload channel emotes").clicked() {
                if let Some(ch) = self.channels.get_mut(&channel) {
                  match ch {
                    Channel::Twitch { twitch, shared } => {
                      if let Some(room_id) = twitch.room_id.as_ref() && let Err(e) = self.emote_loader.tx.try_send(EmoteRequest::TwitchBadgeEmoteListRequest { 
                        channel_id: room_id.to_owned(), 
                        channel_name: shared.channel_name.to_owned(),
                        token: self.auth_tokens.twitch_auth_token.to_owned(), 
                        force_redownload: true
                      }) {  
                        warn!("Failed to send load emote json request for channel {channel} due to error {e:?}");
                      }
                    },
                    Channel::DGG { dgg, shared } => {
                      if let Err(e) = self.emote_loader.tx.try_send(EmoteRequest::DggFlairEmotesRequest { 
                        channel_name: shared.channel_name.to_owned(),
                        cdn_base_url: dgg.dgg_cdn_url.to_owned(),
                        force_redownload: true
                      }) {
                        error!("Failed to load badge/emote json for channel {channel} due to error {e:?}");
                      }
                    },
                    Channel::Youtube { youtube: _, shared: _ } => {}
                  };
                }
                self.show_channel_options = None;
              }
              if ui.button("Split screen").clicked() {
                if self.selected_channel.as_ref() == Some(&channel) {
                    self.selected_channel = if self.rhs_selected_channel.is_some() {
                        self.rhs_selected_channel.to_owned()
                    } else { None };
                }
                self.rhs_selected_channel = Some(channel.to_owned());
                self.last_frame_ui_events.push_back(UiEvent::ChannelChangeRHS);
                self.show_channel_options = None;
              }
            } else {
              let channels = self.channels.iter_mut();
              ui.label("Show mentions from:");
              for (name, channel) in channels {
                ui.checkbox(&mut channel.shared_mut().show_in_mentions_tab, name);
              }
            }
          }).unwrap_or_log();
          if ctx.input(|i| i.pointer.any_click())
              && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
              && !add_menu.response.rect.contains(pos) {
            self.show_channel_options = None;
          }
          else if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.show_channel_options = None;
          }
        }
        channel_removed
      }    
}