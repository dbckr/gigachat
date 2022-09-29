/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{collections::HashMap, time::Duration, sync::Arc, cell::Cell};

use async_channel::Receiver;
use chrono::Utc;
use itertools::Itertools;
use warp::{Filter, reply};
use tokio::{runtime::Runtime, sync::Mutex};
use tracing::{error, log::{warn, info}};
use super::{OutgoingMessage, IncomingMessage, ChatMessage, ChatManager, UserProfile, ProviderName};

pub fn start_listening(runtime: &Runtime) -> ChatManager {
  let (mut out_tx, out_rx) = async_channel::unbounded::<IncomingMessage>();
  let (in_tx, mut in_rx) = async_channel::unbounded::<OutgoingMessage>();

  let mut out_tx_2 = out_tx.clone();
  let mut in_rx_2 = in_rx.clone();
  let handle = runtime.spawn(async move { 
    let outgoing_msg = warp::get()
    .and(warp::path("outgoing-msg"))
    .map(move || in_rx_2.clone())
    .and_then(|rx: Receiver<OutgoingMessage>| async move {
      match tokio::time::timeout(Duration::from_millis(10000), rx.recv()).await {
        Ok(msg) => match msg {
          Ok(msg) => match msg {
            OutgoingMessage::Chat { channel_name, message } => {
              //let msg = format!("{{ \"message\": \"{}\" }}", message);
              let msg = OutgoingMsgResponse { message };
              Ok(reply::json(&msg))
            },
            _ => Ok(reply::json(&"{}"))
          },
          Err(e) => { error!("{}", e); Err(warp::reject::reject()) }
        },
        Err(e) => Ok(reply::json(&"{}"))
      }
    });
    let incoming_msg = warp::post()
    .and(warp::path("incoming-msg"))
    .and(warp::body::content_length_limit(1024 * 64))
    .and(warp::body::json())
    .map( move |request: IncomingMsgRequest| {
      println!("{}", &request.message);
      match out_tx_2.try_send(IncomingMessage::PrivMsg { 
        message: ChatMessage { 
          provider: super::ProviderName::YouTube, 
          //channel: request.get("channel").unwrap_or(&unknown_value).to_owned(), 
          channel: "Youtube".to_owned(),
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
          msg_type: super::MessageType::Chat }
      }) {
        Ok(_) => {

        },
        Err(e) => error!("Failure sending on out_tx: {}", e)
      };

      if let Some(emotes) = request.emotes && emotes.len() > 0 {
        match out_tx_2.try_send(IncomingMessage::MsgEmotes { 
          provider: ProviderName::YouTube, 
          emote_ids: emotes.iter().map(|e| (e.name.to_owned(), e.src.to_owned())).collect_vec()
        }) {
          Ok(_) => (),
          Err(e) => { warn!("Failure sending emote data on out_tx: {}", e); }
        };
      };

      reply::json(&"{}")
    });

    let routes = outgoing_msg.or(incoming_msg);
    warp::serve(routes)
      .run(([127, 0, 0, 1], 8008))
      .await;
  });

  ChatManager { 
    handles: vec![handle], 
    username: "".to_owned(), 
    in_tx: in_tx, 
    out_rx: out_rx 
  }
}

#[derive(serde::Serialize)]
struct OutgoingMsgResponse {
  message: String
}

#[derive(serde::Deserialize)]
struct IncomingMsgRequest {
  message: String,
  username: String,
  role: Option<String>,
  emotes: Option<Vec<IncomingMsgRequestEmote>>
}

#[derive(serde::Deserialize)]
struct IncomingMsgRequestEmote {
  name: String,
  src: String
}