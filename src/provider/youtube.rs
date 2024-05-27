/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{time::Duration};
use egui::Context;
use chrono::{Utc, TimeZone};
use curl::easy::{Easy};
use itertools::Itertools;
use serde_json::Value;
use tokio::{runtime::Runtime, sync::mpsc};

use super::make_request;
use super::{Channel, OutgoingMessage, InternalMessage, ProviderName, ChatMessage, UserProfile, ChannelTransient};

pub fn init_channel<'a>(name: String, channel_id: String, token: String, runtime: &Runtime, ctx: &Context) -> Channel {
  let (out_tx, out_rx) = mpsc::channel::<InternalMessage>(32);
  let (in_tx, mut in_rx) = mpsc::channel::<OutgoingMessage>(32);
  let name_copy = name.to_owned();
  let channel_id_copy = channel_id.to_owned();
  let ctx = ctx.clone();

  let task = runtime.spawn(async move { 
    // periodically poll youtube API for new chat messages
    let mut easy = reqwest::Client::new();
    let mut activelivechat_id : Option<String> = None;
    let mut next_page_token : Option<String> = None;

    loop {
      while let Ok(_) = in_rx.try_recv() { }

      // If channel is streaming get chat messages until no messages found and wait 15 seconds before trying again
      if let Some(activelivechat_id) = &activelivechat_id {
        let (page_token, msgs) = get_chat_messages(
          &token, 
          activelivechat_id, 
          &name_copy, 
          match next_page_token { Some(x) => { info!("using page token {}", x); x}, None => "".to_owned() }, 
          &mut easy);
        info!("updating token to {:?}", page_token);
        next_page_token = page_token;
        if let Some(messages) = msgs && messages.len() > 0 {
          info!("got {} messages", &messages.len());
          for message in messages {
            if let Err(e) = out_tx.try_send(InternalMessage::PrivMsg { message: message }) {
              info!("Error sending PrivMsg: {}", e);
            }
            ctx.request_repaint();
          }
        }
        else { 
          // no new messages
          tokio::time::sleep(Duration::from_millis(10000)).await;
        }
        tokio::time::sleep(Duration::from_millis(20000)).await;
      }
      else {
        // Check if channel is live streaming
        // If not wait a minute before checking again
        if let Some(video_id) = check_channel(&token, &channel_id_copy, &mut easy) {
          _ = out_tx.try_send(InternalMessage::StreamingStatus { is_live: true });
          activelivechat_id = get_active_livestreamchat_id(&token, &video_id, &mut easy);
          if activelivechat_id.is_none() {
            tokio::time::sleep(Duration::from_millis(60000)).await;  
          }
        }
        else {
          activelivechat_id = None;
          _ = out_tx.try_send(InternalMessage::StreamingStatus { is_live: false });
          tokio::time::sleep(Duration::from_millis(60000)).await;
          continue;
        }
      }
    }
  });

  let channel = Channel {  
    provider: ProviderName::YouTube,  
    channel_name: name.to_owned(),
    roomid: channel_id.to_owned(),
    send_history: Default::default(),
    send_history_ix: None,
    transient: Some(ChannelTransient {
      channel_emotes: None,
      badge_emotes: None,
      is_live: false
    })
  };
  channel
}

fn check_channel(token: &String, channel_id: &String, easy : &mut Easy) -> Option<String> {
  let url = format!("https://youtube.googleapis.com/youtube/v3/search?part=id&channelId={}&eventType=live&maxResults=5&type=video", channel_id);
  let headers = Some([("Authorization", format!("Bearer {}", token)), ("Accept", "application/json".to_owned())].to_vec());
  match make_request(&url, headers, easy) {
    Ok(data) => {
      match serde_json::from_str::<Value>(&data){
        Ok(v) => v["items"][0]["id"]["videoId"].as_str().and_then(|x| Some(x.to_owned())),
        Err(e) => { info!("JSON Error: {}", e); None }
      }
    }
    Err(e) => { info!("Error: {}", e); None }
  }
}

fn get_active_livestreamchat_id(token: &String, video_id: &String, easy : &mut Easy) -> Option<String> {
  let url = format!("https://youtube.googleapis.com/youtube/v3/videos?part=liveStreamingDetails&id={}", video_id);
  let headers = Some([("Authorization", format!("Bearer {}", token)), ("Accept", "application/json".to_owned())].to_vec());
  match make_request(&url, headers, easy) {
    Ok(data) => {
      match serde_json::from_str::<Value>(&data){
        Ok(v) => v["items"][0]["liveStreamingDetails"]["activeLiveChatId"].as_str().and_then(|x| Some(x.to_owned())),
        Err(e) => { info!("JSON Error: {}", e); None }
      }
    }
    Err(e) => { info!("Error: {}", e); None }
  }
}

fn get_chat_messages(token: &String, livestreamchat_id: &String, channel_name: &String, next_page_token: String, easy : &mut Easy) -> (Option<String>, Option<Vec<ChatMessage>>) {
  let url = format!("https://youtube.googleapis.com/youtube/v3/liveChat/messages?liveChatId={}&part=snippet,authorDetails&maxResults=100&pageToken={}", livestreamchat_id, next_page_token);
  let headers = Some([("Authorization", format!("Bearer {}", token)), ("Accept", "application/json".to_owned())].to_vec());
  match make_request(&url, headers, easy) {
    Ok(data) => {
      info!("{}", data);
      match serde_json::from_str::<Value>(&data){
        Ok(v) => {
          let token = v["nextPageToken"].as_str().and_then(|x| Some(x.to_owned()));
          (token, Some(v["items"].as_array().unwrap_or_log().into_iter().filter_map(|item| { Some(ChatMessage { 
            provider: ProviderName::YouTube, 
            channel: channel_name.to_owned(), 
            username: item["authorDetails"]["displayName"].as_str()
              .and_then(|x| Some(x.to_owned()))
              .or_else(|| Some("unknown1".to_owned())).unwrap_or_log(), 
            timestamp: item["snippet"]["publishedAt"].as_str().and_then(|x| Some(Utc.datetime_from_str(x, "%+").unwrap_or_log())).or_else(|| Some(Utc::now())).unwrap_or_log().into(),
            message: item["snippet"]["displayMessage"].as_str()
              .and_then(|x| Some(x.to_owned()))
              .or_else(|| Some("".to_owned())).unwrap_or_log(), 
            ..Default::default() }) }).collect_vec()))
        },
        Err(e) => { info!("JSON Error: {}", e); (None, None) }
      }
    }
    Err(e) => { info!("Error: {}", e); (None, None) }
  }
}