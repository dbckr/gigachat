/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//use std::time::Duration;

use std::time::Duration;

use async_channel::{Receiver, Sender};
use chrono::Utc;
use itertools::Itertools;
use warp::{Filter, reply};
use tokio::runtime::Runtime;
use tracing::{error, log::{warn}};
use super::{OutgoingMessage, IncomingMessage, ChatMessage, ChatManager, UserProfile, ProviderName};

pub fn start_listening(runtime: &Runtime) -> ChatManager {
  let (out_tx, out_rx) = async_channel::unbounded::<IncomingMessage>();
  let (in_tx, in_rx) = async_channel::unbounded::<OutgoingMessage>();
  //let (z_tx, z_rx) =  async_channel::unbounded::<OutgoingMessage>();

  let in_tx_2 = in_tx.clone();
  let handle = runtime.spawn(async move { 
    let outgoing_msg = warp::get()
    .and(warp::path("outgoing-msg"))
    .and(warp::path::param())
    .map(move |requesting_channel: String| (requesting_channel, in_rx.clone(), in_tx_2.clone()))
    .and_then(|(requesting_channel, in_rx, in_tx): (String, Receiver<OutgoingMessage>, Sender<OutgoingMessage>)| async move {
      if let Ok(requesting_channel) = urlencoding::decode(&requesting_channel).map(|x| x.into_owned()) {
        match tokio::time::timeout(Duration::from_millis(10000), in_rx.recv()).await {
          Ok(msg) => match msg {
          //match in_rx.try_recv() {
            Ok(msg) => match msg {
              OutgoingMessage::Chat { channel, message } => {
                if channel.trim_start_matches("YT:") == requesting_channel {
                  let msg = OutgoingMsgResponse { channel: channel.trim_start_matches("YT:").to_owned(), message };
                  Ok(reply::json(&msg))
                }
                else {
                  match in_tx.try_send(OutgoingMessage::Chat { channel, message }) {
                    Ok(_) => {},
                    Err(e) => { warn!("Error requeuing OutgoingMessage: {}", e); }
                  };
                  Ok(reply::json(&"{}"))
                }
              },
              _ => Ok(reply::json(&"{}"))
            },
            Err(e) => { error!("{}", e); Err(warp::reject::reject()) }
          },
          Err(_) => Ok(reply::json(&"{}"))
        }
      }
      else {
        Ok(reply::json(&"{}"))
      }
    });

    let incoming_msg = warp::post()
    .and(warp::path("incoming-msg"))
    .and(warp::body::content_length_limit(1024 * 64))
    .and(warp::body::json())
    .map(move |request: IncomingMsgRequest| (request, out_tx.clone()))
    .and_then( |(request, out_tx): (IncomingMsgRequest, Sender<IncomingMessage>)| async move {
      println!("{}", &request.message);
      let mut error = false;
      match out_tx.send(IncomingMessage::PrivMsg { 
        message: ChatMessage { 
          provider: super::ProviderName::YouTube, 
          channel: format!("YT:{}", request.channel),
          username: request.username.to_owned(), 
          timestamp: Utc::now(), 
          message: request.message.to_owned(), 
          profile: UserProfile {
            badges: None,
            display_name: Some(request.username.to_owned()),
            color: match request.role.as_deref() {
              Some("moderator") => Some((94, 132, 241)),
              Some("member") => Some((43, 166, 64)),
              _ => Some((186, 186, 186))
            },
          }, 
          combo_data: None, 
          is_removed: None, 
          msg_type: match request.role.as_deref() {
            Some("error") => super::MessageType::Error,
            _ => super::MessageType::Chat 
          },
          ..Default::default()
        }
      }).await {
        Ok(_) => (),
        Err(e) => { error!("Failure sending on out_tx: {}", e); error = true; }
      };

      if let Some(emotes) = request.emotes && !emotes.is_empty() {
        match out_tx.send(IncomingMessage::MsgEmotes { 
          provider: ProviderName::YouTube, 
          emote_ids: emotes.iter().map(|e| (e.name.to_owned(), e.src.to_owned())).collect_vec()
        }).await {
          Ok(_) => (),
          Err(e) => { warn!("Failure sending emote data on out_tx: {}", e); error = true; }
        };
      };

      if error {
        Err(warp::reject::reject())
      }
      else {
        Ok(reply::json(&"{}"))
      }
    });

    let routes = incoming_msg.or(outgoing_msg);
    warp::serve(routes)
      .run(([127, 0, 0, 1], 36969))
      .await;
  });

  ChatManager { 
    handles: vec![handle], 
    username: "".to_owned(), 
    in_tx, 
    out_rx 
  }
}

#[derive(serde::Serialize)]
struct OutgoingMsgResponse {
  channel: String,
  message: String
}

#[derive(serde::Deserialize)]
struct IncomingMsgRequest {
  message: String,
  username: String,
  role: Option<String>,
  emotes: Option<Vec<IncomingMsgRequestEmote>>,
  channel: String
}

#[derive(serde::Deserialize)]
struct IncomingMsgRequestEmote {
  name: String,
  src: String
}