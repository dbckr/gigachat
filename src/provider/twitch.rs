/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use futures::prelude::*;
use irc::client::{prelude::*};
use tokio::{sync::{mpsc}, runtime::Runtime};
use crate::{app::{Channel}, provider::convert_color_hex, emotes::{EmoteLoader}};

use super::{ChatMessage, UserProfile, InternalMessage, OutgoingMessage};

pub fn open_channel<'a>(name : String, runtime : &Runtime, emote_loader: &mut EmoteLoader) -> Channel {
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
    provider: "twitch".to_owned(), 
    channel_name: name.to_string(),
    roomid: rid,
    tx: in_tx,
    rx: out_rx,
    history: Vec::default(),
    history_viewport_size_y: Default::default(),
    channel_emotes: channel_emotes,
    task_handle: Some(task)
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
  let name = format!("#{}", name);
  config.channels.push(name.to_owned());
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
            println!("{:?}", message);
            match message.command {
              Command::PRIVMSG(ref _target, ref msg) => {
                let sender_name = match message.source_nickname() {
                    Some(sn) => sn.to_owned(),
                    _ => "".to_owned()
                  };
    
                  // Parse out tags
                  if let Some(tags) = message.tags {
                    let cmsg = ChatMessage { 
                      username: sender_name.to_owned(), 
                      timestamp: chrono::Utc::now(), 
                      message: msg.to_owned(),
                      profile: get_user_profile(&tags)
                    };
                    match tx.try_send(InternalMessage::PrivMsg { message: cmsg }) {
                      Ok(_) => (),
                      Err(x) => println!("Send failure: {}", x)
                    };
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
            match &message.chars().next() {
              Some(x) if x.to_owned() == ':' => sender.send_privmsg(&name, format!(" {}", &message))?,
              _ => sender.send_privmsg(&name, &message)?,
            };
            let cmsg = ChatMessage { 
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