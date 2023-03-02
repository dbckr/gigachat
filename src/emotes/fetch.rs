/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::{fs::{File, OpenOptions, DirBuilder}, path::Path, io::{Write, BufRead, Read}};
use itertools::Itertools;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tracing::{debug, warn};
use std::io::BufReader;
use super::{Emote};
use tracing_unwrap::{OptionExt, ResultExt};

#[cfg(feature = "instrumentation")]
use tracing::{instrument};

#[allow(dead_code)]
enum EmoteSize {
  Small,
  Medium,
  Large
}

const EMOTE_DOWNLOADSIZE : EmoteSize = EmoteSize::Medium;

#[cfg_attr(feature = "instrumentation", instrument(skip_all))]
pub async fn process_badge_json(
  room_id: &str,
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
  client: &reqwest::Client,
  force_redownload: bool
) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  let data = get_json_from_url(url, Some(filename), headers, client, force_redownload).await?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() { // Twitch Badges
    for set in v["data"].as_array_mut().unwrap_or_log() {
      let set_id = set["set_id"].as_str().unwrap_or_log().to_owned();
      for v in set["versions"].as_array_mut().unwrap_or_log() {
        let id = v["id"].as_str().unwrap_or_log();
        let name = format!("{set_id}/{id}");
        let id = format!("{}__{}__{}", room_id, &set_id, &id);
        let imgurl = v["image_url_4x"].as_str().unwrap_or_log();
        emotes.push(Emote {
          name,
          id,
          url: imgurl.to_owned(),
          path: "twitch-badge/".to_owned(),
          ..Default::default()
        });
      }
    }
  }
  Ok(emotes)
}

#[cfg_attr(feature = "instrumentation", instrument(skip_all))]
pub async fn process_twitch_follower_emote_json(
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
  client: &reqwest::Client,
  force_redownload: bool
) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  //info!("processing emote json {}", filename);
  let data = get_json_from_url(url, Some(filename), headers, client, force_redownload).await?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() {
    // Twitch Global
    for i in v["data"].as_array_mut().unwrap_or_log() {
      if let Some(emote_type) = i["emote_type"].as_str() && emote_type == "follower" { 
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let extension;
        let wtf = i["format"].as_array().unwrap_or_log();
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
        emotes.push(Emote {name, id, url: imgurl, path: "twitch/".to_owned(), extension, ..Default::default()});
      }
    }
  }
  Ok(emotes)
}

#[cfg_attr(feature = "instrumentation", instrument(skip_all))]
pub async fn process_emote_json(
  url: &str,
  filename: &str,
  headers: Option<Vec<(&str, &String)>>,
  client: &reqwest::Client,
  force_redownload: bool
) -> std::result::Result<Vec<Emote>, anyhow::Error> {
  //info!("processing emote json {}", filename);
  let data = get_json_from_url(url, Some(filename), headers, client, force_redownload).await?;
  let mut v: serde_json::Value = serde_json::from_str(&data)?;
  let mut emotes: Vec<Emote> = Vec::default();
  if v["data"].is_array() {
    // Twitch Global
    let emote_size = match EMOTE_DOWNLOADSIZE {
      EmoteSize::Small => "url_1x",
      EmoteSize::Medium => "url_2x",
      EmoteSize::Large => "url_4x"
    };
    for i in v["data"].as_array_mut().unwrap_or_log() {
      let name = i["name"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let extension;
      let wtf = i["format"].as_array().unwrap_or_log();
      let imgurl = if wtf.len() == 2 {
        extension = Some("gif".to_owned());
        i["images"][emote_size]
          .to_string()
          .trim_matches('"')
          .replace("/static/", "/animated/")
          .to_owned()
      } else {
        extension = Some("png".to_owned());
        i["images"][emote_size].to_string().trim_matches('"').to_owned()
      };
      emotes.push(Emote { name, id, url: imgurl, path: "twitch/".to_owned(), extension, ..Default::default()});
    }
  } else if !v["channelEmotes"].is_null() {
    // BTTV
    let emote_size = match EMOTE_DOWNLOADSIZE {
      EmoteSize::Small => "1x",
      EmoteSize::Medium => "2x",
      EmoteSize::Large => "3x"
    };
    for i in v["channelEmotes"].as_array_mut().unwrap_or_log() {
      let name = i["code"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let ext = i["imageType"].to_string().trim_matches('"').to_owned();
      let imgurl = format!("https://cdn.betterttv.net/emote/{}/{}", &id, emote_size);
      emotes.push(Emote {name, id, url: imgurl, path: "bttv/".to_owned(), extension: Some(ext), ..Default::default()});
    }
    for i in v["sharedEmotes"].as_array_mut().unwrap_or_log() {
      let name = i["code"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let ext = i["imageType"].to_string().trim_matches('"').to_owned();
      let imgurl = format!("https://cdn.betterttv.net/emote/{}/{}", &id, emote_size);
      emotes.push(Emote {name, id, url: imgurl, path: "bttv/".to_owned(), extension: Some(ext), ..Default::default()});
    }
  } else if !v["room"].is_null() {
    // FFZ
    let emote_size = match EMOTE_DOWNLOADSIZE {
      EmoteSize::Small => "1",
      EmoteSize::Medium => "2",
      EmoteSize::Large => "4"
    };
    let setid = v["room"]["set"].to_string();
    for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap_or_log() {
      let name = i["name"].to_string().trim_matches('"').to_owned();
      let id = i["id"].to_string().trim_matches('"').to_owned();
      let url_selected = &i["urls"][emote_size];
      let url_fallback = &i["urls"]["1"];
      let url = match url_selected.is_null() {
        true => url_fallback.to_string(),
        false => url_selected.to_string()
      };
      let imgurl = format!(
        "https:{}",
        url.trim_matches('"')
      );
      emotes.push(Emote {name, id, url: imgurl, path: "ffz/".to_owned(), ..Default::default()});
    }
  } else if !v[0].is_null() {
    for i in v.as_array_mut().unwrap_or_log() {
      if !i["code"].is_null() {
        // BTTV Global
        let emote_size = match EMOTE_DOWNLOADSIZE {
          EmoteSize::Small => "1x",
          EmoteSize::Medium => "2x",
          EmoteSize::Large => "3x"
        };
        let name = i["code"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        let ext = i["imageType"].to_string().trim_matches('"').to_owned();
        let imgurl = format!("https://cdn.betterttv.net/emote/{}/{}", &id, emote_size);
        emotes.push(Emote {name, id, url: imgurl, path: "bttv/".to_owned(), extension: Some(ext), ..Default::default()});
      } else if !i["name"].is_null() {
        // 7TV
        let emote_size = match EMOTE_DOWNLOADSIZE {
          EmoteSize::Small => "1",
          EmoteSize::Medium => "2",
          EmoteSize::Large => "4"
        };
        let name = i["name"].to_string().trim_matches('"').to_owned();
        let id = i["id"].to_string().trim_matches('"').to_owned();
        // 7TV just says webp for everything, derp
        //let extension = i["mime"].to_string().trim_matches('"').replace("image/", "");
        //let x = i["urls"].as_array().unwrap_or_log().last().unwrap_or_log().as_array().unwrap_or_log().last().unwrap_or_log().as_str();
        let x = i["urls"].as_array().unwrap_or_log().iter().filter_map(|y| {
          if let Some((key, value)) = y.as_array().unwrap_or_log().iter().collect_tuple() && key.as_str() == Some(emote_size) {
            value.as_str()
          } else {
            None
          }
        }).next();
        let imgurl = x.unwrap_or_log();
        let zero_width = i["visibility_simple"].as_array().map(|f| f.iter().any(|f| f.as_str().unwrap_or_default() == "ZERO_WIDTH")).unwrap_or(false);
        emotes.push(Emote {
          name,
          id,
          url: imgurl.trim_matches('"').to_owned(),
          path: "7tv/".to_owned(),
          extension: None,
          zero_width,
          ..Default::default()
        });
      }
    }
  }

  Ok(emotes)
}

#[cfg_attr(feature = "instrumentation", instrument(skip_all))]
pub async fn get_json_from_url(
  url: &str,
  filename: Option<&str>,
  headers: Option<Vec<(&str, &String)>>,
  client: &reqwest::Client,
  force_redownload: bool
) -> std::result::Result<String, anyhow::Error> {

  let mut buffer: Vec<u8> = Default::default();
  let mut json: String = Default::default();  

  let filename = filename.map(|f| if f.contains('.') { f.to_owned() } else { format!("{f}.json") } );
  let file_exists = filename.as_ref().is_some_and(|f| Path::new(f).exists());

  if force_redownload || !file_exists {
    debug!("Downloading {}", url);
    let mut hmap = HeaderMap::new();
    if let Some(x) = headers { 
      x.iter().for_each(|(h,v)| {
        let v = HeaderValue::from_str(v.to_owned().as_str()).unwrap_or_log();
        hmap.insert(HeaderName::try_from(*h).unwrap_or_log(), v);
      }) 
    }
    //let client = reqwest::Client::new();
    let req = client
      .get(url)
      .headers(hmap);
    let resp = req.send().await?;
    buffer = resp.bytes().await?.to_vec();
  }
  
  if !buffer.is_empty() {
    match std::str::from_utf8(&buffer) {
      Ok(str) => json.push_str(str),
      Err(e) => panic!("{}", e)
    };
  }
  
  if let Some(filename) = filename {
    if !file_exists {
      let path = Path::new(&filename);
      if let Some(parent_path) = path.parent() {
        DirBuilder::new().recursive(true).create(parent_path)?;
      }
    }

    if file_exists && !force_redownload {
      let name = filename.to_owned();
      let file = File::open(filename)?;
      for line in BufReader::new(file).lines().enumerate().filter_map(|(ix, result)| result.inspect_err(|err| warn!("Failed to parse line {} from file {} due to error: {:?}", &ix, &name, err)).ok()) {
        json.push_str(&line);
      }
    }
    else {
      let mut f = OpenOptions::new().write(true).create(true).truncate(true).open(&filename)?;
      f.write_all(&buffer)?;
    }
  }

  Ok(json)
}

pub async fn get_binary_from_url(
  url: &str,
  filename: Option<&str>,
  headers: Option<Vec<(&str, &String)>>,
) -> std::result::Result<Vec<u8>, anyhow::Error> {

  let mut buffer: Vec<u8> = Default::default();

  if filename.is_none() || filename.as_ref().is_some_and(|f| !Path::new(f).exists()) {
    debug!("Downloading {}", url);
    let mut hmap = HeaderMap::new();
    if let Some(x) = headers { 
      x.iter().for_each(|(h,v)| {
        let v = HeaderValue::from_str(v.to_owned().as_str()).unwrap_or_log();
        hmap.insert(HeaderName::try_from(*h).unwrap_or_log(), v);
      }) 
    }
    let client = reqwest::Client::new();
    let req = client
      .get(url)
      .headers(hmap);
    let resp = req.send().await?;
    buffer = resp.bytes().await?.to_vec();
  }

  if let Some(filename) = filename {
    let filename = filename.to_string();
    let path = Path::new(&filename);
    if let Some(parent_path) = path.parent() {
      DirBuilder::new().recursive(true).create(parent_path)?;
    }

    if path.exists()
    {
      let file = File::open(&filename).expect_or_log("no such file");
      BufReader::new(file).read_to_end(&mut buffer).expect_or_log("Failed to read file");
    }
    else {
      let mut f = OpenOptions::new().create(true).write(true).open(&filename).expect_or_log("Unable to open file");
      f.write_all(&buffer).expect_or_log("Failed to write to file");
    }
  }

  debug!("Loaded {} bytes from {:?}", buffer.len(), filename);

  Ok(buffer)
}
