/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{fs::{File, OpenOptions, DirBuilder}, path::Path, io::{Write, BufRead, Read}};
use curl::easy::Easy;
use tracing::{debug, warn};
use std::io::BufReader;
use super::{Emote};
use crate::error_util::{LogErrResult, LogErrOption};

pub fn process_badge_json(
  room_id: &str,
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
  force_redownload: bool
) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  let data = get_json_from_url(url, Some(filename), headers, force_redownload)?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() { // Twitch Badges
    for set in v["data"].as_array_mut().log_unwrap() {
      let set_id = set["set_id"].as_str().log_unwrap().to_owned();
      for v in set["versions"].as_array_mut().log_unwrap() {
        let id = v["id"].as_str().log_unwrap();
        let name = format!("{}/{}", set_id, id);
        let id = format!("{}__{}__{}", room_id, &set_id, &id);
        let imgurl = v["image_url_4x"].as_str().log_unwrap();
        emotes.push(Emote {
          name,
          id,
          url: imgurl.to_owned(),
          path: "cache/twitch-badge/".to_owned(),
          ..Default::default()
        });
      }
    }
  }
  Ok(emotes)
}

pub fn process_twitch_follower_emote_json(
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
  force_redownload: bool
) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  //info!("processing emote json {}", filename);
  let data = get_json_from_url(url, Some(filename), headers, force_redownload)?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() {
    // Twitch Global
    for i in v["data"].as_array_mut().log_unwrap() {
      if let Some(emote_type) = i["emote_type"].as_str() && emote_type == "follower" { 
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let extension;
        let wtf = i["format"].as_array().log_unwrap();
        let imgurl = if wtf.len() == 2 {
          extension = Some("gif".to_owned());
          i["images"]["url_4x"]
            .to_string()
            .trim_matches('"')
            .replace("/static/", "/animated/")
            .to_owned()
        } else {
          extension = Some("png".to_owned());
          i["images"]["url_4x"].to_string().trim_matches('"').to_owned()
        };
        emotes.push(Emote {name, id, url: imgurl, path: "cache/twitch/".to_owned(), extension, ..Default::default()});
      }
    }
  }
  Ok(emotes)
}

pub fn process_emote_json(
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
  force_redownload: bool
) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  //info!("processing emote json {}", filename);
  let data = get_json_from_url(url, Some(filename), headers, force_redownload)?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() {
    // Twitch Global
    for i in v["data"].as_array_mut().log_unwrap() {
      let name = i["name"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let extension;
      let wtf = i["format"].as_array().log_unwrap();
      let imgurl = if wtf.len() == 2 {
        extension = Some("gif".to_owned());
        i["images"]["url_4x"]
          .to_string()
          .trim_matches('"')
          .replace("/static/", "/animated/")
          .to_owned()
      } else {
        extension = Some("png".to_owned());
        i["images"]["url_4x"].to_string().trim_matches('"').to_owned()
      };
      emotes.push(Emote { name, id, url: imgurl, path: "cache/twitch/".to_owned(), extension, ..Default::default()});
    }
  } else if v["channelEmotes"].is_null() == false {
    // BTTV
    for i in v["channelEmotes"].as_array_mut().log_unwrap() {
      let name = i["code"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let ext = i["imageType"].to_string().trim_matches('"').to_owned();
      let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
      emotes.push(Emote {name, id, url: imgurl, path: "cache/bttv/".to_owned(), extension: Some(ext), ..Default::default()});
    }
    for i in v["sharedEmotes"].as_array_mut().log_unwrap() {
      let name = i["code"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let ext = i["imageType"].to_string().trim_matches('"').to_owned();
      let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
      emotes.push(Emote {name, id, url: imgurl, path: "cache/bttv/".to_owned(), extension: Some(ext), ..Default::default()});
    }
  } else if v["room"].is_null() == false {
    // FFZ
    let setid = v["room"]["set"].to_string();
    for i in v["sets"][&setid]["emoticons"].as_array_mut().log_unwrap() {
      let name = i["name"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let imgurl = format!(
        "https:{}",
        i["urls"].as_object_mut().log_unwrap().values().last().log_unwrap().to_string().trim_matches('"')
      );
      emotes.push(Emote {name, id, url: imgurl, path: "cache/ffz/".to_owned(), ..Default::default()});
    }
  } else if v[0].is_null() == false {
    for i in v.as_array_mut().log_unwrap() {
      if i["code"].is_null() == false {
        // BTTV Global
        let name = i["code"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let ext = i["imageType"].to_string().trim_matches('"').to_owned();
        let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
        emotes.push(Emote {name, id, url: imgurl, path: "cache/bttv/".to_owned(), extension: Some(ext), ..Default::default()});
      } else if i["name"].is_null() == false {
        // 7TV
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        // 7TV just says webp for everything, derp
        //let extension = i["mime"].to_string().trim_matches('"').replace("image/", "");
        let x = i["urls"].as_array().log_unwrap().last().log_unwrap().as_array().log_unwrap().last().log_unwrap();
        let imgurl = x.as_str().log_unwrap();
        let zero_width = i["visibility_simple"].as_array().map(|f| f.iter().any(|f| f.as_str().unwrap_or_default() == "ZERO_WIDTH")).unwrap_or(false);
        emotes.push(Emote {
          name,
          id,
          url: imgurl.trim_matches('"').to_owned(),
          path: "cache/7tv/".to_owned(),
          extension: None,
          zero_width,
          ..Default::default()
        });
      }
    }
  }

  Ok(emotes)
}

pub fn get_json_from_url(
  url: &str,
  filename: Option<&str>,
  headers: Option<Vec<(&str, &String)>>,
  _force_redownload: bool
) -> std::result::Result<String, anyhow::Error> {

  let mut buffer: Vec<u8> = Default::default();
  let mut json: String = Default::default();  

  if /*force_redownload ||*/ filename.is_none() || filename.is_some_and(|f| Path::new(&format!("{}.json", f)).exists() == false) {
    let mut easy = Easy::new();
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
      for byte in data {
        buffer.push(byte.to_owned());
      }
      Ok(data.len())
    })?;
    transfer.perform()?;
  }
  
  if !buffer.is_empty() {
    match std::str::from_utf8(&buffer) {
      Ok(str) => json.push_str(str),
      Err(e) => panic!("{}", e)
    };
  }

  if let Some(filename) = filename {
    let filename = format!("{}.json", filename);
    let path = Path::new(&filename);
    if let Some(parent_path) = path.parent() {
      DirBuilder::new().recursive(true).create(parent_path)?;
    }

    if path.exists()
    {
      let name = filename.to_owned();
      let file = File::open(filename)?;
      for line in BufReader::new(file).lines().enumerate().filter_map(|(ix, result)| result.inspect_err(|err| warn!("Failed to parse line {} from file {} due to error: {:?}", &ix, &name, err)).ok()) {
        json.push_str(&line);
      }
    }
    else {
      let mut f = OpenOptions::new().create_new(true).write(true).open(&filename)?;
      f.write_all(&buffer)?;
    }
  }

  Ok(json)
}

pub fn get_binary_from_url(
  url: &str,
  filename: Option<&str>,
  headers: Option<Vec<(&str, &String)>>,
) -> std::result::Result<Vec<u8>, anyhow::Error> {

  let mut buffer: Vec<u8> = Default::default();

  if filename.is_none() || filename.is_some_and(|f| Path::new(f.to_owned()).exists() == false) {
    let mut easy = Easy::new();
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
      for byte in data {
        buffer.push(byte.to_owned());
      }
      Ok(data.len())
    })?;
    transfer.perform()?;
  }

  if let Some(filename) = filename {
    let filename = filename.to_string();
    let path = Path::new(&filename);
    if let Some(parent_path) = path.parent() {
      DirBuilder::new().recursive(true).create(parent_path)?;
    }

    if path.exists()
    {
      let file = File::open(&filename).log_expect("no such file");
      BufReader::new(file).read_to_end(&mut buffer).log_expect("Failed to read file");
    }
    else {
      let mut f = OpenOptions::new().create_new(true).write(true).open(&filename).log_expect("Unable to open file");
      f.write_all(&buffer).log_expect("Failed to write to file");
    }
  }

  debug!("Loaded {} bytes from {:?}", buffer.len(), filename);

  Ok(buffer)
}
