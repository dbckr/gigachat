use futures::prelude::*;
use irc::client::prelude::*;
use tokio::{sync::mpsc, runtime::Runtime};
use crate::{app::{Channel, ChatMessage, UserProfile}, provider::convert_color_hex};

pub fn open_channel<'a>(name : String, runtime : &Runtime) -> Channel {
  let (tx, rx) = mpsc::channel(32);
  let channel = Channel {  
    provider: "twitch".to_owned(), 
    label: name.to_owned(),
    rx: rx,
    history: Vec::default()
  };
  let _task = runtime.spawn(async move { spawn_irc(name, tx).await });
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
  sender.send_cap_req(&[Capability::Custom("twitch.tv/tags"), Capability::Custom("twitch.tv/commands")])?;
  while let Some(message) = stream.next().await.transpose()? {
      print!("{}", message);

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
                      Some(x) => x.to_owned(),
                      None => "".to_owned()
                    },
                    color: convert_color_hex(color_tag),
                    ..Default::default()
                  }
                };
                match tx.try_send(cmsg) {
                  Ok(_) => (),
                  Err(x) => println!("Send failure: {}", x)
                };
              }
          },
          Command::PING(ref target, ref _msg) => {
              sender.send_pong(target)?;
          },
          _ => (),
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