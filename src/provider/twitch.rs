use std::{thread::spawn, net::ToSocketAddrs};

use eframe::egui;
use futures::prelude::*;
use irc::client::prelude::*;
use tokio::{sync::mpsc, runtime::Runtime};

use crate::app::{Channel, ChatMessage};

pub fn open_channel<'a>(name : &String, runtime : &Runtime) -> Channel {
  let (tx, mut rx) = mpsc::channel(32);
  let channel = Channel::new(name, &"twitch".to_string(), rx);
  let n = name.to_owned();
  let _task = runtime.spawn(async move { spawn_irc(n, tx).await });
  channel
}

async fn spawn_irc(name : String, tx : mpsc::Sender<ChatMessage>) -> std::result::Result<(), failure::Error> {
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
  while let Some(message) = stream.next().await.transpose()? {
      print!("{}", message);

      match message.command {
          Command::PRIVMSG(ref _target, ref msg) => {
            let sender_name = match message.source_nickname() {
                Some(sn) => sn,
                _ => ""
              };
              let cmsg = ChatMessage::new(&name, sender_name, chrono::Utc::now(), msg);
              match tx.try_send(cmsg) {
                Ok(_) => (),
                Err(x) => println!("Send failure: {}", x),
                _ => println!("Send unknown.")
              };
          },
          Command::PING(ref target, ref _msg) => {
              sender.send_pong(target)?;
          },
          _ => (),
      }
  }

  Ok(())
}