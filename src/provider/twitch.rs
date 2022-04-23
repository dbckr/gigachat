/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::io::Read;

use futures::prelude::*;
use irc::client::{prelude::*};
use itertools::Itertools;
use tokio::{sync::{mpsc}, runtime::Runtime};
use crate::{provider::{Channel, convert_color_hex, ProviderName}, emotes::{EmoteLoader}};

use super::{ChatMessage, UserProfile, InternalMessage, OutgoingMessage, Provider};

pub fn load_token() -> String {
  let mut result : String = Default::default();
  _ = std::fs::File::open("config/twitchkey").unwrap().read_to_string(&mut result);
  result
}

pub fn open_channel<'a>(name : String, runtime : &Runtime, emote_loader: &mut EmoteLoader, provider: &mut Provider) -> Channel {
  let (out_tx, mut out_rx) = mpsc::channel::<InternalMessage>(32);
  let (in_tx, in_rx) = mpsc::channel::<OutgoingMessage>(32);
  let name2 = name.to_owned();

  let task = runtime.spawn(async move { 
    match spawn_irc(name2, out_tx, in_rx).await {
      Ok(()) => (),
      Err(e) => { println!("Error in twitch thread: {}", e); }
    }
  });
  let rid;

  loop {
    if let Ok(msg) = out_rx.try_recv() {
      match msg {
        InternalMessage::RoomId { room_id } => {
          rid = room_id;
          break;
        },
        InternalMessage::EmoteSets { emote_sets } => {
          for set in emote_sets {
            if provider.emote_sets.contains_key(&set) == false && let Some(set_list) = emote_loader.twitch_get_emote_set(&set) {
              provider.emote_sets.insert(set.to_owned(), set_list);
            }
          }
        },
        _ => ()
      }
    }
  }

  let channel_emotes = match emote_loader.load_channel_emotes(&rid) {
    Ok(x) => x,
    Err(x) => { 
      println!("ERROR LOADING CHANNEL EMOTES: {}", x); 
      Default::default()
    }
  };

  let channel = Channel {  
    provider: ProviderName::Twitch, 
    channel_name: name.to_string(),
    roomid: rid,
    tx: in_tx,
    rx: out_rx,
    history: Vec::default(),
    channel_emotes: channel_emotes,
    task_handle: Some(task),
    is_live: false
  };
  channel
}

async fn spawn_irc(name : String, tx : mpsc::Sender<InternalMessage>, mut rx: mpsc::Receiver<OutgoingMessage>) -> Result<(), failure::Error> {
  let config_path = match std::env::var("IRC_Config_File") {
    Ok(val) => val,
    Err(_e) => "config/irc.toml".to_owned()
  };

  let mut profile = UserProfile::default();

  let mut config = Config::load(config_path)?;
  let name = name.to_owned();
  config.channels.push(format!("#{}", name.to_owned()));
  let mut client = Client::from_config(config).await?;
  client.identify()?;
  let mut stream = client.stream()?;
  let sender = client.sender();
  sender.send_cap_req(&[Capability::Custom("twitch.tv/tags"), Capability::Custom("twitch.tv/commands")])?;
  loop {
    tokio::select! {
      Some(result) = stream.next()  => {
        match result {
          Ok(message) => {
            //println!("{}", message);
            match message.command {
              Command::PRIVMSG(ref _target, ref msg) => {
                let sender_name = match message.source_nickname() {
                    Some(sn) => sn.to_owned(),
                    _ => "".to_owned()
                  };
    
                  // Parse out tags
                  if let Some(tags) = message.tags {
                    let cmsg = ChatMessage { 
                      provider: ProviderName::Twitch,
                      channel: name.to_owned(),
                      username: sender_name.to_owned(), 
                      timestamp: chrono::Utc::now(), 
                      message: msg.trim_end_matches(['\u{e0000}', '\u{1}']).to_owned(),
                      profile: get_user_profile(&tags)
                    };
                    match tx.try_send(InternalMessage::PrivMsg { message: cmsg }) {
                      Ok(_) => (),
                      Err(x) => println!("Send failure: {}", x)
                    };
                    if let Some(emote_ids) = get_tag_value(&tags, "emotes") && emote_ids.len() > 0 {
                      let ids = emote_ids.split(",").filter_map(|x| {
                        let pair = x.split(":").collect_vec();
                        if pair.len() < 2 { return None; }
                        let range = pair[1].split("-").filter_map(|x| match x.parse::<usize>() { Ok(x) => Some(x), Err(x) => None } ).collect_vec();
                        match range.len() {
                          2 => Some((pair[0].to_owned(), msg[range[0]..=range[1]].to_owned())),
                          _ => None
                        }
                      }).to_owned().collect_vec();
                      tx.try_send(InternalMessage::MsgEmotes { 
                        emote_ids: ids });
                    }
                  }
              },
              Command::PING(ref target, ref _msg) => {
                  sender.send_pong(target)?;
              },
              Command::Raw(ref command, ref _str_vec) => {
                if let Some(tags) = message.tags {
                  if command == "USERSTATE" {
                    profile = get_user_profile(&tags);
                    tx.try_send(InternalMessage::EmoteSets { 
                      emote_sets: get_tag_value(&tags, "emote-sets").unwrap().split(",").map(|x| x.to_owned()).collect::<Vec<String>>() });
                  }
                  else if command == "ROOMSTATE" {
                    tx.try_send(InternalMessage::RoomId { 
                      room_id: get_tag_value(&tags, "room-id").unwrap().to_owned() });
                  }
                  else {
                    ()
                  }
                }
                else {
                  ()
                }
              },
              _ => ()
          }
          },
          Err(e) => println!("{:?}", e)
        }
      },
      Some(out_msg) = rx.recv() => {
        match out_msg {
          OutgoingMessage::Chat { message } => { 
            /*match &message.chars().next() {
              Some(x) if x.to_owned() == ':' => sender.send_privmsg(&name, format!(" {}", &message))?,
              _ => sender.send_privmsg(&name, &message)?,
            };*/
            let cmsg = ChatMessage { 
              provider: ProviderName::Twitch,
              channel: name.to_owned(),
              username: client.current_nickname().to_owned(), 
              timestamp: chrono::Utc::now(), 
              message: message, 
              profile: profile.to_owned()
            };
            match tx.try_send(InternalMessage::PrivMsg { message: cmsg }) {
              Ok(_) => (),
              Err(x) => println!("Send failure: {}", x)
            };
          },
          OutgoingMessage::Leave {  } => return Ok(())
        };
      }
    };
  }
}

fn get_user_profile(tags: &Vec<irc::proto::message::Tag>) -> UserProfile {
  UserProfile {
    display_name: get_tag_value(&tags, "display-name"),
    color: convert_color_hex(get_tag_value(&tags, "color").as_ref()),
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

pub struct TwitchToken {
  token : String
}

impl std::fmt::Display for TwitchToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.token)
    }
}

pub fn authenticate(runtime : &Runtime) {
  let client_id = "fpj6py15j5qccjs8cm7iz5ljjzp1uf";
  let scope = "";
  let state = format!("{}", rand::random::<u128>());
  let authorize_url = format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=https://dbckr.github.io/GigachatAuth&response_type=token&scope={}&state={}", client_id, scope, state);

  open::that(authorize_url);
}