use std::thread::spawn;

use futures::prelude::*;
use irc::client::prelude::*;
use tokio::{sync::mpsc, runtime::Runtime};

use super::{Channel, ChatMessage};

pub fn open_channel(name : &String) -> Channel {
  print!("tokio runtime");
  let runtime = Runtime::new().expect("new tokio Runtime");
  let (tx, mut rx) = mpsc::channel(32);
  let channel = Channel::new(name, rx);
  print!("spawn irc");
  let task = runtime.spawn(spawn_irc(name.to_string(), tx));
  print!("return channel");
  channel
}

async fn spawn_irc(name : String, tx : mpsc::Sender<ChatMessage>) -> std::result::Result<(), failure::Error> {
  let config_path = match std::env::var("IRC_Config_File") {
    Ok(val) => val,
    Err(_e) => "config/irc.toml".to_owned()
  };

  let config = Config::load(config_path)?;
  let mut client = Client::from_config(config).await?;
  client.identify()?;
  let mut stream = client.stream()?;
  let sender = client.sender();
  while let Some(message) = stream.next().await.transpose()? {
      print!("{}", message);

      match message.command {
          Command::PRIVMSG(ref target, ref msg) => {
              let sender_name = message.source_nickname().unwrap();

              // add message to list
              tx.send(ChatMessage::new(&name, sender_name, chrono::Utc::now(), msg));
          },
          Command::PING(ref target, ref _msg) => {
              sender.send_pong(target)?;
          },
          _ => (),
      }
  }

  Ok(())
}