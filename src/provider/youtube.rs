use std::{time::Duration};
 
use chrono::{Utc, TimeZone};
use curl::easy::{Easy};
use itertools::Itertools;
use serde_json::Value;
use tokio::{runtime::Runtime, sync::mpsc};

use super::{Channel, OutgoingMessage, InternalMessage, ProviderName, ChatMessage, UserProfile, ChannelTransient};

pub fn init_channel<'a>(name: String, channel_id: String, token: String, runtime: &Runtime) -> Channel {
  let (out_tx, out_rx) = mpsc::channel::<InternalMessage>(32);
  let (in_tx, mut in_rx) = mpsc::channel::<OutgoingMessage>(32);
  let name_copy = name.to_owned();
  let channel_id_copy = channel_id.to_owned();

  let task = runtime.spawn(async move { 
    // periodically poll youtube API for new chat messages
    let mut easy = Easy::new();
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
          match next_page_token { Some(x) => { println!("using page token {}", x); x}, None => "".to_owned() }, 
          &mut easy);
        println!("updating token to {:?}", page_token);
        next_page_token = page_token;
        if let Some(messages) = msgs && messages.len() > 0 {
          println!("got {} messages", &messages.len());
          for message in messages {
            if let Err(e) = out_tx.try_send(InternalMessage::PrivMsg { message: message }) {
              println!("Error sending PrivMsg: {}", e);
            }
          }
        }
        else { 
          println!("no new messages");
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
    transient: Some(ChannelTransient {
      tx: in_tx,
      rx: out_rx,
      channel_emotes: None,
      badge_emotes: None,
      task_handle: task,
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
        Err(e) => { println!("JSON Error: {}", e); None }
      }
    }
    Err(e) => { println!("Error: {}", e); None }
  }
}

fn get_active_livestreamchat_id(token: &String, video_id: &String, easy : &mut Easy) -> Option<String> {
  let url = format!("https://youtube.googleapis.com/youtube/v3/videos?part=liveStreamingDetails&id={}", video_id);
  let headers = Some([("Authorization", format!("Bearer {}", token)), ("Accept", "application/json".to_owned())].to_vec());
  match make_request(&url, headers, easy) {
    Ok(data) => {
      match serde_json::from_str::<Value>(&data){
        Ok(v) => v["items"][0]["liveStreamingDetails"]["activeLiveChatId"].as_str().and_then(|x| Some(x.to_owned())),
        Err(e) => { println!("JSON Error: {}", e); None }
      }
    }
    Err(e) => { println!("Error: {}", e); None }
  }
}

fn get_chat_messages(token: &String, livestreamchat_id: &String, channel_name: &String, next_page_token: String, easy : &mut Easy) -> (Option<String>, Option<Vec<ChatMessage>>) {
  let url = format!("https://youtube.googleapis.com/youtube/v3/liveChat/messages?liveChatId={}&part=snippet,authorDetails&maxResults=100&pageToken={}", livestreamchat_id, next_page_token);
  let headers = Some([("Authorization", format!("Bearer {}", token)), ("Accept", "application/json".to_owned())].to_vec());
  match make_request(&url, headers, easy) {
    Ok(data) => {
      println!("{}", data);
      match serde_json::from_str::<Value>(&data){
        Ok(v) => {
          let token = v["nextPageToken"].as_str().and_then(|x| Some(x.to_owned()));
          (token, Some(v["items"].as_array().unwrap().into_iter().filter_map(|item| { Some(ChatMessage { 
            provider: ProviderName::YouTube, 
            channel: channel_name.to_owned(), 
            username: item["authorDetails"]["displayName"].as_str()
              .and_then(|x| Some(x.to_owned()))
              .or_else(|| Some("unknown1".to_owned())).unwrap(), 
            timestamp: item["snippet"]["publishedAt"].as_str().and_then(|x| Some(Utc.datetime_from_str(x, "%+").unwrap())).or_else(|| Some(Utc::now())).unwrap().into(),
            message: item["snippet"]["displayMessage"].as_str()
              .and_then(|x| Some(x.to_owned()))
              .or_else(|| Some("".to_owned())).unwrap(), 
            profile: UserProfile { ..Default::default() } }) }).collect_vec()))
        },
        Err(e) => { println!("JSON Error: {}", e); (None, None) }
      }
    }
    Err(e) => { println!("Error: {}", e); (None, None) }
  }
}

fn make_request(url: &String, headers: Option<Vec<(&str, String)>>, easy : &mut Easy) -> Result<String, failure::Error> {
  let mut result = String::default();

    easy.url(url)?;
    if let Some(headers) = headers {
      let mut list = curl::easy::List::new();
      for head in headers {
        list.append(&format!("{}: {}", head.0, head.1))?;
      }
      easy.http_headers(list)?;
    }
    let mut transfer = easy.transfer();
    transfer.write_function(|data| { 
      String::from_utf8(data.to_vec()).and_then(|x| Ok((&mut result).push_str(&x))).expect("failed to build string from http response body");
      Ok(data.len())
    })?;
    transfer.perform()?;
    drop(transfer);

    Ok(result)
}