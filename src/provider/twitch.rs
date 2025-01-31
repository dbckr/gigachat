/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::{HashSet, HashMap};
use async_channel::{Receiver, Sender};
use backoff::backoff::Backoff;
use egui::Context;
use tracing::{info, trace, error, debug};
use chrono::{DateTime, Utc};
use futures::prelude::*;
use irc::client::prelude::*;
use itertools::Itertools;
use tokio::{runtime::Runtime, time::sleep, time::Duration};
use crate::{provider::{convert_color_hex, ProviderName, ChannelStatus, MessageType}, emotes::fetch::get_json_from_url};
use tracing_unwrap::{OptionExt, ResultExt};
use super::{ChatMessage, UserProfile, IncomingMessage, OutgoingMessage, ChatManagerRx, channel::{Channel, ChannelTransient, ChannelShared, TwitchChannel}};

const TWITCH_STATUS_FETCH_INTERVAL_SEC : i64 = 60;

struct TwitchChannelData {
    room_id: Option<String>,
    show_offline_chat: bool
}

pub struct TwitchChatManager {
  handle: tokio::task::JoinHandle<()>,
  pub username: String,
  pub in_tx: Sender<OutgoingMessage>,
  pub out_rx: Receiver<IncomingMessage>,
}

impl TwitchChatManager {
  pub fn new(username: &String, token: &String, runtime: &Runtime, ctx: &Context) -> Self {
    let (out_tx, out_rx) = async_channel::bounded::<IncomingMessage>(10000);
    let (in_tx, in_rx) = async_channel::bounded::<OutgoingMessage>(10000);
    let token2 = token.to_owned();
    let name2 = username.to_owned();
    let ctx = ctx.clone();

    let task = runtime.spawn(async move { 
      let mut backoff = backoff::ExponentialBackoffBuilder::new()
      .with_initial_interval(Duration::from_millis(3000))
      .with_max_interval(Duration::from_millis(60000))
      .with_max_elapsed_time(None)
      .with_randomization_factor(0.)
      .build();

      let mut channels_joined : HashMap<String,TwitchChannelData> = Default::default();
      loop {
        let retry_wait = backoff.next_backoff();
        match spawn_irc(&name2, &token2, &out_tx, &in_rx, &mut channels_joined, &ctx).await {
          Ok(x) => if x { break; } else { 
            backoff.reset(); 
            backoff.next_backoff();
            super::display_system_message_in_chat(
              &out_tx, 
              String::new(), 
              ProviderName::Twitch, 
              format!("Lost connection, retrying in {:.3?} seconds...", retry_wait.map(|x| x.as_secs_f32())),
              MessageType::Error, &ctx);
          },
          Err(e) => { 
            error!("Failed to connect to twitch irc: {:?}", e);
            //super::display_system_message_in_chat(&out_tx, String::new(), ProviderName::Twitch, format!("Error: {}", e), MessageType::Error);
            super::display_system_message_in_chat(
              &out_tx, 
              String::new(), 
              ProviderName::Twitch, 
              format!("Failed to reconnect, retrying in {:.3?} seconds...", retry_wait.map(|x| x.as_secs_f32())), 
              MessageType::Error, &ctx);
          }
        }
        if let Some(duration) = retry_wait {
          sleep(duration).await;
        }
      }
    });

    Self {
        username: username.to_owned(),
        handle: task,
        in_tx,
        out_rx,
    }
  }

  pub fn leave_channel(&mut self, channel_name : &String) {
    self.in_tx.try_send(OutgoingMessage::Leave { channel_name: channel_name.to_owned() }).expect_or_log("channel failure");
  }

  pub fn close(&mut self) {
    if !self.handle.is_finished() {
      self.in_tx.try_send(OutgoingMessage::Quit {}).expect_or_log("channel failure");
    }
  }

  pub fn init_channel(&mut self, channel_name : &str) -> Channel {
    let mut channel = Channel::Twitch { 
      shared: ChannelShared {
        channel_name: channel_name.to_lowercase(),
        show_in_mentions_tab: true,
        show_tab_when_offline: false,
        send_history: Default::default(),
        send_history_ix: None,
        transient: None,
        users: Default::default()
      },
      twitch: TwitchChannel {
        room_id: Default::default()
      }
    };
    if let Channel::Twitch { twitch, shared } = &mut channel {
        self.open_channel(twitch, shared);
    }
    
    channel
  }

  pub fn open_channel(&mut self, twitch: &TwitchChannel, shared: &mut ChannelShared) {
    if shared.transient.is_none() {
            shared.transient = Some(ChannelTransient {
            channel_emotes: None,
            badge_emotes: None,
            status: None
        });
    }

    self.in_tx.try_send(OutgoingMessage::TwitchJoin{ channel_name: shared.channel_name.to_owned(), room_id: twitch.room_id.clone(), show_offline_chat: shared.show_tab_when_offline }).expect_or_log("channel failure");
  }
}

impl ChatManagerRx for TwitchChatManager {
  fn in_tx(&mut self) -> &mut Sender<OutgoingMessage> {
    &mut self.in_tx
  }
  fn out_rx(&mut self) -> &mut Receiver<IncomingMessage> {
    &mut self.out_rx
  }
}

async fn spawn_irc(user_name : &String, token: &String, tx : &Sender<IncomingMessage>, rx: &Receiver<OutgoingMessage>, channels: &mut HashMap<String,TwitchChannelData>, ctx: &Context) -> Result<bool, anyhow::Error> {
  let web_client_builder = reqwest::Client::builder()
      .timeout(Duration::from_secs(30));
  let web_client = web_client_builder.build().unwrap_or_log();

  let mut profiles : HashMap<String, UserProfile> = Default::default();
  let mut client = Client::from_config(Config { 
      username: Some(user_name.to_owned()),
      nickname: Some(user_name.to_owned()),  
      server: Some("irc.chat.twitch.tv".to_owned()), 
      port: Some(6697), 
      password: Some(format!("oauth:{token}")), 
      use_tls: Some(true),
      ping_time: Some(180),
      ping_timeout: Some(90),
      ..Default::default()
    }).await?;
  client.identify()?;
  let mut stream = client.stream()?;
  let sender = client.sender();
  sender.send_cap_req(&[Capability::Custom("twitch.tv/tags"), Capability::Custom("twitch.tv/commands")]).expect_or_log("failed to send cap req");

  //super::display_system_message_in_chat(tx, String::new(), ProviderName::Twitch, "Connected to chat.".to_owned(), MessageType::Information);

  let mut joined_channels : HashMap<String, bool> = Default::default();
  let mut seen_emote_ids : HashSet<String> = Default::default();
  let mut last_status_check : Option<DateTime<Utc>> = None;
  let mut last_ping_received : DateTime<Utc> = Utc::now();

  // If reconnecting, rejoin any previously joined channels
  // for (channel, _) in channels.iter() {
  //    client.send_join(format!("#{channel}")).expect_or_log("failed to join channel");
  //}

  //sender.send_join(format!("#{}", name.to_owned())).expect_or_log("failed to join channel");
  
  loop {

    //TODO: split this out to a separate thread
    // check channel statuses
    if last_status_check.is_none() || last_status_check.is_some_and(|f| Utc::now().signed_duration_since(f.to_owned()).num_milliseconds() > TWITCH_STATUS_FETCH_INTERVAL_SEC * 1000) {
      let room_ids = channels.values().filter_map(|x| x.room_id.as_ref()).collect_vec();
      if !room_ids.is_empty() {
        last_status_check = Some(Utc::now());
        let status_data = get_channel_statuses(room_ids, token, &web_client).await;

        for (channel, channel_data) in channels.iter() {

          let status_update_msg = if let Some(status) = status_data.iter().find(|x| Some(&x.user_id) == channel_data.room_id.as_ref()) {
            ChannelStatus {
              game_name: Some(status.game_name.to_owned()),
              is_live: matches!(status.stream_type.as_str(), "live"),
              title: Some(status.title.to_owned()),
              viewer_count: Some(status.viewer_count),
              started_at: Some(status.started_at.to_owned()),
            }
          }
          else {
            ChannelStatus {
              game_name: None,
              is_live: false,
              title: None,
              viewer_count: None,
              started_at: None,
            }
          };

          if (status_update_msg.is_live || channel_data.show_offline_chat) && joined_channels.get(channel).unwrap_or(&false) == &false {
            join(&client, tx, &mut joined_channels, channel, ctx);
          } else if !status_update_msg.is_live && !channel_data.show_offline_chat && joined_channels.get(channel).unwrap_or(&false) == &true {
            leave(&client, tx, &mut joined_channels, channel, ctx);
          }

          if let Err(e) = tx.try_send(IncomingMessage::StreamingStatus { channel: channel.to_lowercase().to_owned(), status: Some(status_update_msg)}) {
            info!("error sending status: {}", e)
          }
        }
        ctx.request_repaint();
      }
    }

    tokio::select! {
      Some(result) = stream.next()  => {
        match result {
          Ok(message) => {
            trace!("{}", message);
            match message.command {
              Command::PRIVMSG(ref _target, ref msg) => {
                let sender_name = match message.source_nickname() {
                  Some(sn) => sn.to_owned(),
                  _ => "".to_owned()
                };
                //let channel = message.
                // Parse out tags
                if let Some(tags) = message.tags.as_ref() {
                  let cmsg = ChatMessage { 
                    provider: ProviderName::Twitch,
                    channel: _target.trim_start_matches('#').to_owned(),
                    username: sender_name.to_owned(),
                    //tmi-sent-ts
                    timestamp: get_tag_value(tags, "tmi-sent-ts")
                      .and_then(|x| x.parse::<usize>().ok())
                      .and_then(|x| DateTime::from_timestamp(x as i64 / 1000, (x % 1000 * 1000_usize.pow(2)) as u32 ))
                      //.map(|x| x.)
                      .unwrap_or_else(chrono::Utc::now),
                    message: msg.trim_end_matches(['\u{e0000}', '\u{1}']).to_owned(),
                    profile: get_user_profile(tags),
                    ..Default::default()
                  };
                  if let Some(emote_ids) = get_tag_value(tags, "emotes") && !emote_ids.is_empty() {
                    //info!("{}", message);
                    let ids = emote_ids.split('/').filter_map(|x| {
                      let pair = x.split(':').collect_vec();
                      if pair.len() < 2 { return None; }
                      if seen_emote_ids.contains(pair[0]) {
                        return None;
                      } else {
                        seen_emote_ids.insert(pair[0].to_owned());
                      }
                      let range = pair[1].split(',').next()
                        .map(|r| r.split('-').filter_map(|x| match x.parse::<usize>() { Ok(x) => Some(x), Err(_x) => None } ).collect_vec())
                        .unwrap_or_default();
                      match range.len() {
                        //2 => Some((pair[0].to_owned(), msg[range[0]..=range[1]].to_owned())),
                        2 => { 
                          let x : String = msg.to_owned().chars().collect_vec().iter().skip(range[0]).take(range[1] - range[0] + 1).collect();
                          Some((pair[0].to_owned(), x))
                        },
                        _ => None
                      }
                    }).sorted_by_key(|(_a, b)| b.to_owned()).dedup().collect_vec();
                    if let Err(e) = tx.try_send(IncomingMessage::MsgEmotes { provider: ProviderName::Twitch, emote_ids: ids }) {
                      info!("Error sending MsgEmotes: {}", e);
                    }
                  }
                  match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
                    Ok(_) => (),
                    Err(x) => info!("Send failure: {}", x)
                  };
                }
              },
              Command::PING(ref target, ref msg) => {
                  info!("received PING: {:?} | {:?}", target, msg);
                  last_ping_received = Utc::now();
                  //sender.send_pong(message).expect_or_log("failed to send pong");
              },
              Command::PONG(ref target, ref msg) => {
                  info!("received PONG: {:?} | {:?}", target, msg);
              },
              Command::Raw(ref command, ref str_vec) => {
                //trace!("Recieved Twitch IRC Command: {}", command);
                if let Some(tags) = message.tags {
                  let channel_name = str_vec.last().unwrap_or_log().trim_start_matches('#');
                  let result = match command.as_str() {
                    "USERSTATE" => {
                      let channel = channel_name.to_owned();
                      profiles.insert(channel, get_user_profile(&tags));
                      tx.try_send(IncomingMessage::EmoteSets { 
                        provider: ProviderName::Twitch,
                        emote_sets: get_tag_value(&tags, "emote-sets").unwrap_or_log().split(',').map(|x| x.to_owned()).collect::<Vec<String>>() 
                      })
                    },
                    "ROOMSTATE" => {
                      if let Some(channel_data) = channels.get_mut(channel_name) && let Some(roomid) = get_tag_value(&tags, "room-id") {
                        channel_data.room_id = Some(roomid);
                      }

                      // small delay to not spam twitch API when joining channels at app start
                      last_status_check = Some(Utc::now() - chrono::Duration::milliseconds(TWITCH_STATUS_FETCH_INTERVAL_SEC * 1000 - 250));
                      tx.try_send(IncomingMessage::RoomId { 
                        channel: channel_name.to_owned(),
                        room_id: get_tag_value(&tags, "room-id").unwrap_or_log().to_owned() })
                    },
                    "NOTICE" => {
                        tx.try_send(IncomingMessage::PrivMsg { message: ChatMessage { 
                          provider: ProviderName::Twitch, 
                          channel: channel_name.to_owned(), 
                          timestamp: chrono::Utc::now(), 
                          message: str_vec.join(", ").to_string(),
                          msg_type: MessageType::Error,
                          ..Default::default()
                        }})
                    },
                    "USERNOTICE" => {
                      if let Some(sys_msg) = get_tag_value(&tags, "system-msg") {
                        tx.try_send(IncomingMessage::PrivMsg { message: ChatMessage { 
                          provider: ProviderName::Twitch, 
                          channel: channel_name.to_owned(), 
                          timestamp: chrono::Utc::now(), 
                          message: sys_msg,
                          msg_type: MessageType::Announcement,
                          ..Default::default()
                        }})
                      } else {
                        Ok(())
                      }
                    },
                    "CLEARCHAT" => {
                      tx.try_send(IncomingMessage::UserMuted { 
                        channel: channel_name.to_owned(), 
                        username: str_vec[1].to_owned() })
                    },
                    _ => { debug!("unknown IRC command: {} {}", command, str_vec.join(", ")); Ok(())}
                  };
                  if let Err(e) = result {
                    info!("IRC Raw error: {}", e);
                  }
                }
              },
              _ => debug!("Unknown message type: {:?}", message)
            }
            ctx.request_repaint();
          },
          Err(e) => { 
            error!("Twitch IRC error: {:?}", e);
            return Ok(false);
          }
        }
      },
      Ok(out_msg) = rx.recv() => {
        match out_msg {
          OutgoingMessage::Chat { channel, message } => { 
            _ = match &message.chars().next() {
              Some(x) if *x == ':' => sender.send_privmsg(&channel, format!(" {}", &message)),
              _ => sender.send_privmsg(&format!("#{channel}"), &message),
            }.inspect_err(|e| { info!("Error sending twitch IRC message: {}", e)});
            let profile = profiles.get(&channel).map(|f| f.to_owned()).unwrap_or_default();
            let cmsg = ChatMessage { 
              provider: ProviderName::Twitch,
              channel,
              username: client.current_nickname().to_owned(), 
              timestamp: chrono::Utc::now(), 
              message, 
              profile,
              ..Default::default()
            };
            match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
              Ok(_) => (),
              Err(x) => info!("Send failure: {}", x)
            };
            ctx.request_repaint();
          },
          OutgoingMessage::Quit {  } => { client.send_quit("Leaving").expect_or_log("Error while quitting IRC server"); info!("quit command received"); return Ok(true); },
          OutgoingMessage::Leave { channel_name } => {
            leave(&client, tx, &mut joined_channels, &channel_name, ctx);
          },
          OutgoingMessage::Join { channel_name: _ } => {},
          OutgoingMessage::TwitchJoin { channel_name, room_id, show_offline_chat } => {
            let has_room_id = room_id.is_some();
            channels.insert(channel_name.to_owned(), TwitchChannelData { room_id, show_offline_chat });
        
            // Join chat to get the roomid (needed for status checks)
            if !has_room_id || show_offline_chat {
                join(&client, tx, &mut joined_channels, &channel_name, ctx);
            }
          }
        };
      },
      _ = tokio::time::sleep(Duration::from_secs(3)) => {
        if last_ping_received.checked_add_signed(chrono::Duration::minutes(10)).unwrap_or_log() < Utc::now() {
            error!("IRC is unresponsive, reconnecting...");
            super::display_system_message_in_chat(tx, String::new(), ProviderName::Twitch, "IRC is unresponsive, reconnecting...".to_owned(), MessageType::Error, ctx);
            //return Err(anyhow::Error::msg("Twitch IRC is hanging. Restarting..."));
            return Ok(false);
        }
      }
    };

  }
}

fn join(client: &Client, tx: &Sender<IncomingMessage>, joined_channels: &mut HashMap<String, bool>, channel: &String, ctx: &Context) {
    client.send_join(format!("#{channel}")).expect_or_log("failed to join channel");
    super::display_system_message_in_chat(tx, channel.to_owned(), ProviderName::Twitch, format!("Joined {channel} chat."), MessageType::Information, ctx);
    joined_channels.insert(channel.to_owned(), true);
}

fn leave(client: &Client, tx: &Sender<IncomingMessage>, joined_channels: &mut HashMap<String, bool>, channel: &String, ctx: &Context) {
    client.send_part(format!("#{channel}")).expect_or_log("failed to leave channel");
    super::display_system_message_in_chat(tx, channel.to_owned(), ProviderName::Twitch, format!("Leaving {channel} chat."), MessageType::Information, ctx);
    joined_channels.insert(channel.to_owned(), false);
}

fn get_user_profile(tags: &Vec<irc::proto::message::Tag>) -> UserProfile {
  UserProfile {
    display_name: get_tag_value(tags, "display-name"),
    color: convert_color_hex(get_tag_value(tags, "color").as_ref()),
    badges: get_tag_value(tags, "badges").map(|b| b.split(',').filter_map(|x| if !x.is_empty() { Some(x.to_owned()) } else { None }).collect_vec())
  }
}

fn get_tag_value(tags: &Vec<irc::proto::message::Tag>, key: &str) -> Option<String> {
  for tag in tags {
    if tag.0 == key {
      return tag.1.to_owned();
    }
  }
  None
}

pub fn authenticate() -> String {
  let client_id = "fpj6py15j5qccjs8cm7iz5ljjzp1uf";
  let scope = "chat:read chat:edit";
  let state = format!("{}", rand::random::<u128>());
  format!("https://id.twitch.tv/oauth2/authorize?client_id={client_id}&redirect_uri=https://dbckr.github.io/GigachatAuth&response_type=token&scope={scope}&state={state}")
}

async fn get_channel_statuses(channel_ids : Vec<&String>, token: &String, client: &reqwest::Client) -> Vec<TwitchChannelStatus> {
  if channel_ids.is_empty() {
    return Default::default();
  }
  let url = format!("https://api.twitch.tv/helix/streams?{}", channel_ids.iter().map(|f| format!("user_id={f}")).collect_vec().join("&"));
  let json = match get_json_from_url(&url, None, Some([
    ("Authorization", &format!("Bearer {token}")),
    ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())].to_vec()), client, true).await {
      Ok(json) => json,
      Err(e) => { error!("failed getting twitch statuses: {}", e); return Default::default(); }
    };
  //info!("{}", json);
  parse_channel_status_json(channel_ids, json)
}

pub fn parse_channel_status_json(channel_ids: Vec<&String>, json: String) -> Vec<TwitchChannelStatus> {
  let result: Result<TwitchChannelStatuses, _> = serde_json::from_str(&json);
  match result {
    Ok(result) => {
      channel_ids.iter().filter_map(|cid| { 
        result.data.iter().find(|i| &&i.user_id == cid).map(|i| i.to_owned())
      }).collect_vec()
    },
    Err(e) => { info!("error deserializing channel statuses: {}", e); Default::default() }
  }
}
#[derive(serde::Deserialize)]
pub struct TwitchChannelStatuses {
  data: Vec<TwitchChannelStatus>
}

#[derive(Clone,Debug,Default)]
#[derive(serde::Deserialize)]
pub struct TwitchChannelStatus {
  pub user_id: String,
  pub user_name: String,
  pub game_name: String,
  #[serde(alias = "type")]
  pub stream_type: String,
  pub title: String,
  pub viewer_count: usize,
  pub started_at: String
}