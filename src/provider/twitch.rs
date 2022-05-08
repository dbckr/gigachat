/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::HashSet};

use futures::prelude::*;
use irc::client::{prelude::*};
use itertools::Itertools;
use tokio::{sync::{mpsc}, runtime::Runtime};
use crate::{provider::{Channel, convert_color_hex, ProviderName}, emotes::{EmoteLoader}};

use super::{ChatMessage, UserProfile, IncomingMessage, OutgoingMessage, ChannelTransient};

pub struct TwitchChatManager {
  token: String,
  handle: tokio::task::JoinHandle<()>,
  pub in_tx: mpsc::Sender<OutgoingMessage>,
  pub out_rx: mpsc::Receiver<IncomingMessage>,
}

impl TwitchChatManager {

  pub fn new(username: &String, token: &String, runtime: &Runtime) -> Self {
    let (out_tx, out_rx) = mpsc::channel::<IncomingMessage>(256);
    let (in_tx, in_rx) = mpsc::channel::<OutgoingMessage>(32);
    let token2 = token.to_owned();
    let name2 = username.to_owned();

    let task = runtime.spawn(async move { 
      spawn_irc(name2, token2, out_tx, in_rx).await
    });

    let token = token.to_owned();
    Self {
        token,
        handle: task,
        in_tx,
        out_rx,
    }
  }

  pub fn leave_channel(&mut self, channel_name : &String) {
    self.in_tx.try_send(OutgoingMessage::Leave { channel_name: channel_name.to_owned() }).expect("channel failure");
  }

  pub fn close(&mut self) {
    self.in_tx.try_send(OutgoingMessage::Quit {}).expect("channel failure");
    std::thread::sleep(std::time::Duration::from_millis(1000));
    self.handle.abort();
  }

  pub fn init_channel(&mut self, channel_name : &String, emote_loader: &EmoteLoader) -> Channel {
    let mut channel = Channel {  
      provider: ProviderName::Twitch, 
      channel_name: channel_name.to_owned(),
      roomid: Default::default(),
      send_history: Default::default(),
      send_history_ix: None,
      transient: None
    };
    self.open_channel(&mut channel, emote_loader);
    channel
  }

  pub fn open_channel<'a>(&mut self, channel: &mut Channel, emote_loader: &EmoteLoader) {
    channel.transient = Some(ChannelTransient {
      channel_emotes: None,
      badge_emotes: None,
      is_live: false
    });
    self.in_tx.try_send(OutgoingMessage::Join{ channel_name: channel.channel_name.to_owned() }).expect("channel failure");
  }
}

async fn spawn_irc(user_name : String, token: String, tx : mpsc::Sender<IncomingMessage>, mut rx: mpsc::Receiver<OutgoingMessage>) {
  let mut profile = UserProfile::default();
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
    }).await.expect("failed to create irc client");
  client.identify().expect("failed to identify");
  let mut stream = client.stream().expect("failed to get stream");
  let sender = client.sender();
  sender.send_cap_req(&[Capability::Custom("twitch.tv/tags"), Capability::Custom("twitch.tv/commands")]).expect("failed to send cap req");
  //sender.send_join(format!("#{}", name.to_owned())).expect("failed to join channel");
  let mut seen_emote_ids : HashSet<String> = Default::default();
  let mut quitted = false;
  while !quitted {
    tokio::select! {
      Some(result) = stream.next()  => {
        match result {
          Ok(message) => {
            println!("{}", message);
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
                    channel: _target.trim_start_matches("#").to_owned(),
                    username: sender_name.to_owned(), 
                    timestamp: chrono::Utc::now(), 
                    message: msg.trim_end_matches(['\u{e0000}', '\u{1}']).to_owned(),
                    profile: get_user_profile(&tags),
                    ..Default::default()
                  };
                  if let Some(emote_ids) = get_tag_value(&tags, "emotes") && emote_ids.len() > 0 {
                    //println!("{}", message);
                    let ids = emote_ids.split("/").filter_map(|x| {
                      let pair = x.split(":").collect_vec();
                      if pair.len() < 2 { return None; }
                      if seen_emote_ids.contains(pair[0]) {
                        return None;
                      } else {
                        seen_emote_ids.insert(pair[0].to_owned());
                      }
                      let range = pair[1].split(",").next()
                        .and_then(|r| Some(r.split("-").filter_map(|x| match x.parse::<usize>() { Ok(x) => Some(x), Err(_x) => None } ).collect_vec()))
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
                      println!("Error sending MsgEmotes: {}", e);
                    }
                  }
                  match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
                    Ok(_) => (),
                    Err(x) => println!("Send failure: {}", x)
                  };
                }
              },
              Command::PING(ref target, ref _msg) => {
                  sender.send_pong(target).expect("failed to send pong");
              },
              Command::Raw(ref command, ref str_vec) => {
                println!("Recieved Twitch IRC Command: {}", command);
                if let Some(tags) = message.tags {
                  let result = match command.as_str() {
                    "USERSTATE" => {
                      profile = get_user_profile(&tags);
                      tx.try_send(IncomingMessage::EmoteSets { 
                        provider: ProviderName::Twitch,
                        emote_sets: get_tag_value(&tags, "emote-sets").unwrap().split(",").map(|x| x.to_owned()).collect::<Vec<String>>() 
                      })
                    },
                    "ROOMSTATE" => {
                      tx.try_send(IncomingMessage::RoomId { 
                        channel: str_vec.last().unwrap().trim_start_matches("#").to_owned(),
                        room_id: get_tag_value(&tags, "room-id").unwrap().to_owned() })
                    },
                    "NOTICE" => {
                      if str_vec.contains(&"Login unsuccessful".to_string()) {
                        //panic!("Failed to login to IRC");
                        tx.try_send(IncomingMessage::PrivMsg { message: ChatMessage { 
                          provider: ProviderName::Twitch, 
                          channel: "".to_owned(), 
                          username: "SYSTEM_MSG".to_owned(), 
                          timestamp: chrono::Utc::now(), 
                          message: format!("{}", str_vec.join(", ")), 
                          profile: UserProfile { 
                            color: (255, 0, 0),
                            ..Default::default() 
                          },
                          ..Default::default()
                        }})
                      }
                      else {
                        Ok(())
                      }
                    },
                    _ => { println!("unknown IRC command: {} {}", command, str_vec.join(", ")); Ok(())}
                  };
                  if let Err(e) = result {
                    println!("IRC Raw error: {}", e);
                  }
                }
              },
              _ => ()
          }
          },
          Err(e) => println!("IRC Stream error: {:?}", e)
        }
      },
      Some(out_msg) = rx.recv() => {
        match out_msg {
          OutgoingMessage::Chat { channel_name, message } => { 
            /*_ = match &message.chars().next() {
              Some(x) if x.to_owned() == ':' => sender.send_privmsg(&name, format!(" {}", &message)),
              _ => sender.send_privmsg(&format!("#{name}"), &message),
            }.inspect_err(|e| { println!("Error sending twitch IRC message: {}", e)});*/
            let cmsg = ChatMessage { 
              provider: ProviderName::Twitch,
              channel: channel_name,
              username: client.current_nickname().to_owned(), 
              timestamp: chrono::Utc::now(), 
              message: message, 
              profile: profile.to_owned(),
              ..Default::default()
            };
            match tx.try_send(IncomingMessage::PrivMsg { message: cmsg }) {
              Ok(_) => (),
              Err(x) => println!("Send failure: {}", x)
            };
          },
          OutgoingMessage::Quit {  } => { client.send_quit("Leaving").expect("Error while quitting IRC server"); quitted = true; },
          OutgoingMessage::Leave { channel_name } => {
            client.send_part(format!("#{}", channel_name)).expect("failed to leave channel");
          },
          OutgoingMessage::Join { channel_name } => {
            client.send_join(format!("#{}", channel_name)).expect("failed to leave channel");
          }
        };
      }
    };
  }
}

fn get_user_profile(tags: &Vec<irc::proto::message::Tag>) -> UserProfile {
  UserProfile {
    display_name: get_tag_value(&tags, "display-name"),
    color: convert_color_hex(get_tag_value(&tags, "color").as_ref()),
    badges: get_tag_value(&tags, "badges").and_then(|b| Some(b.split(",").filter_map(|x| if x.len() > 0 { Some(x.to_owned()) } else { None }).collect_vec())),
    ..Default::default()
  }
}

fn get_tag_value(tags: &Vec<irc::proto::message::Tag>, key: &str) -> Option<String> {
  for tag in tags {
    if tag.0 == key {
      return tag.1.to_owned();
    }
  }
  return None;
}

pub fn authenticate(ctx: &egui::Context, _runtime : &Runtime) {
  let client_id = "fpj6py15j5qccjs8cm7iz5ljjzp1uf";
  let scope = "chat:read chat:edit";
  let state = format!("{}", rand::random::<u128>());
  let authorize_url = format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=https://dbckr.github.io/GigachatAuth&response_type=token&scope={}&state={}", client_id, scope, state);

  ctx.output().open_url(&authorize_url);
}