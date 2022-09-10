/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::{HashSet, HashMap}};
use async_channel::{Receiver, Sender};
use tracing::{info, trace};
use chrono::{DateTime, Utc, NaiveDateTime};
use futures::prelude::*;
use irc::client::{prelude::*};
use itertools::Itertools;
use tokio::{runtime::Runtime};
use crate::{provider::{Channel, convert_color_hex, ProviderName, ChannelStatus, MessageType}, emotes::{fetch::get_json_from_url}};
use tracing_unwrap::{OptionExt, ResultExt};
use super::{ChatMessage, UserProfile, IncomingMessage, OutgoingMessage, ChannelTransient};

const TWITCH_STATUS_FETCH_INTERVAL_SEC : i64 = 60;

pub struct TwitchChatManager {
  handle: tokio::task::JoinHandle<()>,
  pub username: String,
  pub in_tx: Sender<OutgoingMessage>,
  pub out_rx: Receiver<IncomingMessage>,
}

impl TwitchChatManager {

  pub fn new(username: &String, token: &String, runtime: &Runtime) -> Self {
    let (out_tx, out_rx) = async_channel::unbounded::<IncomingMessage>();
    let (in_tx, in_rx) = async_channel::unbounded::<OutgoingMessage>();
    let token2 = token.to_owned();
    let name2 = username.to_owned();

    let task = runtime.spawn(async move { 
      spawn_irc(name2, token2, out_tx, in_rx).await
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
    self.in_tx.try_send(OutgoingMessage::Quit {}).expect_or_log("channel failure");
    std::thread::sleep(std::time::Duration::from_millis(1000));
    self.handle.abort();
  }

  pub fn init_channel(&mut self, channel_name : &str) -> Channel {
    let mut channel = Channel {  
      provider: ProviderName::Twitch, 
      channel_name: channel_name.to_lowercase(),
      show_in_mentions_tab: true,
      roomid: Default::default(),
      send_history: Default::default(),
      send_history_ix: None,
      transient: None,
      users: Default::default()
    };
    self.open_channel(&mut channel);
    channel
  }

  pub fn open_channel(&mut self, channel: &mut Channel) {
    channel.transient = Some(ChannelTransient {
      channel_emotes: None,
      badge_emotes: None,
      status: None
    });
    self.in_tx.try_send(OutgoingMessage::Join{ channel_name: channel.channel_name.to_owned() }).expect_or_log("channel failure");
  }
}

async fn spawn_irc(user_name : String, token: String, tx : Sender<IncomingMessage>, rx: Receiver<OutgoingMessage>) {
  let mut profiles : HashMap<String, UserProfile> = Default::default();
  //let name = channel_name.to_owned();
  //let channels = [format!("#{}", name.to_owned())].to_vec();
  let mut client = Client::from_config(Config { 
      username: Some(user_name.to_owned()),
      nickname: Some(user_name),  
      server: Some("irc.chat.twitch.tv".to_owned()), 
      port: Some(6697), 
      password: Some(format!("oauth:{}", token)), 
      use_tls: Some(true),
      //channels: channels,
      ..Default::default()
    }).await.expect_or_log("failed to create irc client");
  client.identify().expect_or_log("failed to identify");
  let mut stream = client.stream().expect_or_log("failed to get stream");
  let sender = client.sender();
  sender.send_cap_req(&[Capability::Custom("twitch.tv/tags"), Capability::Custom("twitch.tv/commands")]).expect_or_log("failed to send cap req");
  //sender.send_join(format!("#{}", name.to_owned())).expect_or_log("failed to join channel");
  let mut seen_emote_ids : HashSet<String> = Default::default();
  let mut active_room_ids : HashMap<String, String> = Default::default();
  let mut quitted = false;
  let mut last_status_check : Option<DateTime<Utc>> = None;
  while !quitted {

    // check channel statuses
    if last_status_check.is_none() || last_status_check.is_some_and(|f| Utc::now().signed_duration_since(f.to_owned()).num_seconds() > TWITCH_STATUS_FETCH_INTERVAL_SEC) {
      let room_ids = active_room_ids.iter().map(|(_k,v)| v).collect_vec();
      if !room_ids.is_empty() {
        last_status_check = Some(Utc::now());
        let status_data = get_channel_statuses(room_ids, &token);

        for (channel, room_id) in active_room_ids.iter() {
          let status_update_msg = if let Some(status) = status_data.iter().find(|x| &x.user_id == room_id) {
            IncomingMessage::StreamingStatus { channel: channel.to_lowercase().to_owned(), status: Some(ChannelStatus {
              game_name: Some(status.game_name.to_owned()),
              is_live: match status.stream_type.as_str() { "live" => true, _ => false },
              title: Some(status.title.to_owned()),
              viewer_count: Some(status.viewer_count),
              started_at: Some(status.started_at.to_owned()),
            }) }
          }
          else {
            IncomingMessage::StreamingStatus { channel: channel.to_lowercase().to_owned(), status: Some(ChannelStatus {
              game_name: None,
              is_live: false,
              title: None,
              viewer_count: None,
              started_at: None,
            }) }
          };

          match tx.try_send(status_update_msg) {
            Err(e) => info!("error sending status: {}", e),
            _ => ()
          }
        }
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
                      .map(|x| DateTime::from_utc(NaiveDateTime::from_timestamp(x as i64 / 1000, (x % 1000 * 1000_usize.pow(2)) as u32 ), Utc))
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
              Command::PING(ref target, ref _msg) => {
                  sender.send_pong(target).expect_or_log("failed to send pong");
              },
              Command::Raw(ref command, ref str_vec) => {
                //trace!("Recieved Twitch IRC Command: {}", command);
                if let Some(tags) = message.tags {
                  let result = match command.as_str() {
                    "USERSTATE" => {
                      let channel = str_vec.first().unwrap_or_log().trim_start_matches('#').to_owned();
                      profiles.insert(channel, get_user_profile(&tags));
                      tx.try_send(IncomingMessage::EmoteSets { 
                        provider: ProviderName::Twitch,
                        emote_sets: get_tag_value(&tags, "emote-sets").unwrap_or_log().split(',').map(|x| x.to_owned()).collect::<Vec<String>>() 
                      })
                    },
                    "ROOMSTATE" => {
                      active_room_ids.insert(str_vec.last().unwrap_or_log().trim_start_matches('#').to_owned(), get_tag_value(&tags, "room-id").unwrap_or_log().to_owned());
                      // small delay to not spam twitch API when joining channels at app start
                      last_status_check = Some(Utc::now() - chrono::Duration::seconds(TWITCH_STATUS_FETCH_INTERVAL_SEC - 2));
                      tx.try_send(IncomingMessage::RoomId { 
                        channel: str_vec.last().unwrap_or_log().trim_start_matches('#').to_owned(),
                        room_id: get_tag_value(&tags, "room-id").unwrap_or_log().to_owned() })
                    },
                    "NOTICE" => {
                        tx.try_send(IncomingMessage::PrivMsg { message: ChatMessage { 
                          provider: ProviderName::Twitch, 
                          channel: str_vec.last().unwrap_or_log().trim_start_matches('#').to_owned(), 
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
                          channel: str_vec.last().unwrap_or_log().trim_start_matches('#').to_owned(), 
                          timestamp: chrono::Utc::now(), 
                          message: sys_msg,
                          msg_type: MessageType::Announcement,
                          ..Default::default()
                        }})
                      } else {
                        Ok(())
                      }
                    },
                    _ => { info!("unknown IRC command: {} {}", command, str_vec.join(", ")); Ok(())}
                  };
                  if let Err(e) = result {
                    info!("IRC Raw error: {}", e);
                  }
                }
              },
              _ => ()
            }
          },
          Err(e) => info!("IRC Stream error: {:?}", e)
        }
      },
      Ok(out_msg) = rx.recv() => {
        match out_msg {
          OutgoingMessage::Chat { channel_name, message } => { 
            _ = match &message.chars().next() {
              Some(x) if *x == ':' => sender.send_privmsg(&channel_name, format!(" {}", &message)),
              _ => sender.send_privmsg(&format!("#{channel_name}"), &message),
            }.inspect_err(|e| { info!("Error sending twitch IRC message: {}", e)});
            let profile = profiles.get(&channel_name).map(|f| f.to_owned()).unwrap_or_default();
            let cmsg = ChatMessage { 
              provider: ProviderName::Twitch,
              channel: channel_name,
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
          },
          OutgoingMessage::Quit {  } => { client.send_quit("Leaving").expect_or_log("Error while quitting IRC server"); quitted = true; },
          OutgoingMessage::Leave { channel_name } => {
            client.send_part(format!("#{}", channel_name.to_owned())).expect_or_log("failed to leave channel");
            active_room_ids.remove(&channel_name);
          },
          OutgoingMessage::Join { channel_name } => {
            client.send_join(format!("#{}", channel_name)).expect_or_log("failed to leave channel");
          }
        };
      }
    };
  }
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

pub fn authenticate(ctx: &egui::Context, _runtime : &Runtime) {
  let client_id = "fpj6py15j5qccjs8cm7iz5ljjzp1uf";
  let scope = "chat:read chat:edit";
  let state = format!("{}", rand::random::<u128>());
  let authorize_url = format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=https://dbckr.github.io/GigachatAuth&response_type=token&scope={}&state={}", client_id, scope, state);

  ctx.output().open_url(&authorize_url);
}

fn get_channel_statuses(channel_ids : Vec<&String>, token: &String) -> Vec<TwitchChannelStatus> {
  if channel_ids.is_empty() {
    return Default::default();
  }
  let url = format!("https://api.twitch.tv/helix/streams?{}", channel_ids.iter().map(|f| format!("user_id={}", f)).collect_vec().join("&"));
  let json = match get_json_from_url(&url, None, Some([
    ("Authorization", &format!("Bearer {}", token)),
    ("Client-Id", &"fpj6py15j5qccjs8cm7iz5ljjzp1uf".to_owned())].to_vec()), true) {
      Ok(json) => json,
      Err(e) => { info!("failed getting twitch statuses: {}", e); return Default::default(); }
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