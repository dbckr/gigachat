use futures::prelude::*;
use irc::client::prelude::*;
use tokio::{sync::mpsc, runtime::Runtime};
use crate::{app::{Channel}, provider::convert_color_hex, emotes::{EmoteLoader}};

use super::{ChatMessage, UserProfile, InternalMessage};

pub fn open_channel<'a>(name : String, runtime : &Runtime, emote_loader: &mut EmoteLoader) -> Channel {
  let (tx, mut rx) = mpsc::channel(32);
  let name2 = name.to_owned();
  let task = runtime.spawn(async move { 
    match spawn_irc(name2, tx).await {
      Ok(()) => (),
      Err(e) => { println!("Error in twitch thread: {}", e); }
    }
  });
  let rid;

  loop {
    if let Some(msg) = rx.blocking_recv() {
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
    rx: rx,
    history: Vec::default(),
    history_viewport_size_y: Default::default(),
    channel_emotes: channel_emotes,
    task_handle: Some(task)
  };
  channel
}

async fn spawn_irc(name : String, tx : mpsc::Sender<InternalMessage>) -> Result<(), failure::Error> {
  let config_path = match std::env::var("IRC_Config_File") {
    Ok(val) => val,
    Err(_e) => "config/irc.toml".to_owned()
  };

  let mut config = Config::load(config_path)?;
  let name = format!("#{}", name);
  config.channels.push(name.to_owned());
  let mut client = Client::from_config(config).await?;
  client.identify()?;
  let mut stream = client.stream()?;
  let sender = client.sender();
  sender.send_cap_req(&[Capability::Custom("twitch.tv/tags"), Capability::Custom("twitch.tv/commands")])?;
  while let Some(message) = stream.next().await.transpose()? {
      //print!("{}", message);

      match message.command {
          Command::PRIVMSG(ref _target, ref msg) => {
            let sender_name = match message.source_nickname() {
                Some(sn) => sn.to_owned(),
                _ => "".to_owned()
              };

              // Parse out tags
              if let Some(tags) = message.tags {
                let color_tag = get_tag_value(&tags, "color");
                let cmsg = ChatMessage { 
                  username: sender_name.to_owned(), 
                  timestamp: chrono::Utc::now(), 
                  message: msg.to_owned(),
                  profile: UserProfile {
                    display_name: match get_tag_value(&tags, "display-name") {
                      Some(x) => Some(x.to_owned()),
                      None => None
                    },
                    color: convert_color_hex(color_tag),
                    ..Default::default()
                  }
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
  }

  Ok(())
}

fn get_tag_value<'a>(tags: &'a Vec<irc::proto::message::Tag>, key: &str) -> Option<&'a String> {
  for tag in tags {
    if tag.0 == key {
      return tag.1.as_ref();
    }
  }
  return None;
}