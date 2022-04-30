/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{fs::{File, OpenOptions, DirBuilder}, path::Path, io::{Write, BufRead}};

use curl::easy::Easy;
use std::io::BufReader;

use super::{Emote, EmoteStatus};

pub fn process_badge_json(
  room_id: &str,
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
) -> std::result::Result<Vec<Emote>, failure::Error> {
  let data = get_emote_json(url, filename, headers)?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() { // Twitch Badges
    for set in v["data"].as_array_mut().unwrap() {
      let set_id = set["set_id"].as_str().unwrap().to_owned();
      for v in set["versions"].as_array_mut().unwrap() {
        let id = v["id"].as_str().unwrap();
        let name = format!("{}/{}", set_id, id);
        let id = format!("{}__{}__{}", room_id, &set_id, &id);
        let imgurl = v["image_url_4x"].as_str().unwrap();
        emotes.push(get_emote(
          name,
          id,
          imgurl.to_owned(),
          "generated/twitch-badge/".to_owned(),
          None
        ));
      }
    }
  }
  Ok(emotes)
}

pub fn process_emote_json(
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
) -> std::result::Result<Vec<Emote>, failure::Error> {
  //println!("processing emote json {}", filename);
  let data = get_emote_json(url, filename, headers)?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() {
    // Twitch Global
    for i in v["data"].as_array_mut().unwrap() {
      let name = i["name"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let extension;
      let wtf = i["format"].as_array().unwrap();
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
      emotes.push(get_emote(name, id, imgurl, "generated/twitch/".to_owned(), extension));
    }
  } else if v["channelEmotes"].is_null() == false {
    // BTTV
    for i in v["channelEmotes"].as_array_mut().unwrap() {
      let name = i["code"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let ext = i["imageType"].to_string().trim_matches('"').to_owned();
      let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
      emotes.push(get_emote(name, id, imgurl, "generated/bttv/".to_owned(), Some(ext)));
    }
    for i in v["sharedEmotes"].as_array_mut().unwrap() {
      let name = i["code"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let ext = i["imageType"].to_string().trim_matches('"').to_owned();
      let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
      emotes.push(get_emote(name, id, imgurl, "generated/bttv/".to_owned(), Some(ext)));
    }
  } else if v["room"].is_null() == false {
    // FFZ
    let setid = v["room"]["set"].to_string();
    for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap() {
      let name = i["name"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let imgurl = format!(
        "https:{}",
        i["urls"].as_object_mut().unwrap().values().last().unwrap().to_string().trim_matches('"')
      );
      emotes.push(get_emote(name, id, imgurl, "generated/ffz/".to_owned(), None));
    }
  } else if v[0].is_null() == false {
    for i in v.as_array_mut().unwrap() {
      // BTTV Global
      if i["code"].is_null() == false {
        let name = i["code"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let ext = i["imageType"].to_string().trim_matches('"').to_owned();
        let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
        emotes.push(get_emote(name, id, imgurl, "generated/bttv/".to_owned(), Some(ext)));
      } else if i["name"].is_null() == false {
        // 7TV
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let extension = i["mime"].to_string().trim_matches('"').replace("image/", "");
        let x = i["urls"].as_array().unwrap().last().unwrap().as_array().unwrap().last().unwrap();
        let imgurl = x.as_str().unwrap();
        emotes.push(get_emote(
          name,
          id,
          imgurl.trim_matches('"').to_owned(),
          "generated/7tv/".to_owned(),
          Some(extension),
        ));
      }
    }
  }

  Ok(emotes)
}

fn get_emote_json(
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
) -> std::result::Result<String, failure::Error> {
  let path = Path::new(filename);
  if path.exists() == false && let Some(parent_path) = path.parent() {
    DirBuilder::new().recursive(true).create(parent_path)?;

    let mut f =
      OpenOptions::new().create_new(true).write(true).open(filename).expect("Unable to open file");

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
      f.write_all(data).expect("Failed to write to file");
      Ok(data.len())
    })?;
    transfer.perform()?;
  }

  let file = File::open(filename).expect("no such file");
  let mut result = String::new();
  for line in BufReader::new(file).lines().filter_map(|result| result.ok()) {
    result.push_str(&line);
  }
  Ok(result)
}


pub fn get_emote(
  name: String,
  id: String,
  url: String,
  path: String,
  extension: Option<String>,
) -> Emote {
  Emote {
    name: name,
    id: id,
    data: None,
    loaded: EmoteStatus::NotLoaded,
    url: url,
    path: path,
    extension,
    duration_msec: 0,
  }
}
