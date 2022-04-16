use curl::easy::{Easy};
use eframe::epaint::ColorImage;
use image::DynamicImage;
use image::imageops::FilterType;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Write, BufReader, BufRead};
use std::fs::{OpenOptions, File};
use std::str;
use failure;
use std::path::Path;

//#[derive(Clone)]
pub struct Emote {
    pub name: String,
    pub data: Option<DynamicImage>
}

pub fn load_channel_emotes(channel_id : &String) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
    let ffz_emotes = process_emote_json(&ffz_url, "generated/ffz-channel-json")?;
    //let bttv_url = format!("https://api.betterttv.net/3/cached/users/twitch/{}", channel_id);
    //process_emote_json(&bttv_url, "generated/bttv-channel-json")?;
    //let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
    //process_emote_json(&seventv_url, "generated/7tv-channel-json")?;

    let mut result : HashMap<String, Emote> = HashMap::new();
    for emote in ffz_emotes {
        result.insert(emote.name.to_owned(), emote);
    }
    Ok(result)
}

pub fn load_global_emotes() -> std::result::Result<HashMap<String, Emote>, failure::Error> {
    let bttv_emotes = process_emote_json("https://api.betterttv.net/3/cached/emotes/global", "generated/bttv-global-json")?;
    let seventv_emotes = process_emote_json("https://api.7tv.app/v2/emotes/global", "generated/7tv-global-json")?;

    let mut result : HashMap<String, Emote> = HashMap::new();

    let unknown_image = image::open("img/DEFAULT.png").unwrap().resize_exact(24, 24, FilterType::Nearest);
    result.insert("_".to_owned(), Emote { name: "DEFAULT".to_owned(), data: Some(unknown_image) });

    for emote in bttv_emotes {
        result.insert(emote.name.to_owned(), emote);
    }
    for emote in seventv_emotes {
        result.insert(emote.name.to_owned(), emote);
    }
    Ok(result)

    //process_twitch_json("config/twitch-json-data")?;
    //process_emote_list("config/twitch-global")?;
    //process_emote_list("config/generic-emoji-list")?;
    //process_emote_list("config/emoji-unicode")?;
}

pub fn load_list_file(filename: &str) -> std::result::Result<Vec<String>, failure::Error> {
    let file = File::open(filename).expect("no such file");
    Ok(BufReader::new(file).lines().map(|l| l.expect("Could not parse line"))
    .collect())
}

pub fn process_emote_list(filename: &str) -> std::result::Result<(), failure::Error> {
    println!("processing emote list {}", filename);

    let mut f = OpenOptions::new()
    .append(true)
    .create(true) // Optionally create the file if it doesn't already exist
    .open("generated/emotes")
    .expect("Unable to open file");

    let file = File::open(filename).expect("no such file");
    for l in BufReader::new(file).lines() {
        let line = l.expect("Could not parse line");
        writeln!(f, "{}", line)?;
    }

    Ok(())
}

pub fn process_emote_json(url: &str, filename: &str) -> std::result::Result<Vec<Emote>, failure::Error> {
    println!("processing emote json {}", filename);
    let data = get_emote_json(url, filename)?;
    let mut v: Value = serde_json::from_str(&data)?;

    let mut emotes : Vec<Emote> = Vec::default();

    if v["channelEmotes"].is_null() == false { // BTTV cache api
        for i in v["channelEmotes"].as_array_mut().unwrap() {
            emotes.push(Emote { name: i["code"].to_string(), data: None })
        }
        for i in v["sharedEmotes"].as_array_mut().unwrap() {
            emotes.push(Emote { name: i["code"].to_string().trim_matches('"').to_owned(), data: None })
        }
    }
    else if v["emotes"].is_null() == false { // BTTV channel name based API
        //e.g. get_emote_json("https://api.betterttv.net/2/channels/jormh", "bttv-jormh-json")?;
        for i in v["emotes"].as_array_mut().unwrap() {
            emotes.push(Emote { name: i["code"].to_string().trim_matches('"').to_owned(), data: None })
        }
    }
    else if v["room"].is_null() == false { // FFZ
        let setid = v["room"]["set"].to_string();
        for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap() {
            emotes.push(Emote { name: i["name"].to_string().trim_matches('"').to_owned(), data: None })
        }
    }
    else if v[0].is_null() == false {
        for i in v.as_array_mut().unwrap() {
            if i["code"].is_null() == false {
                emotes.push(Emote { name: i["code"].to_string().trim_matches('"').to_owned(), data: None })
            }
            else {
                emotes.push(Emote { name: i["name"].to_string().trim_matches('"').to_owned(), data: None })
            }
        }
    }

    Ok(emotes)
}

pub fn process_twitch_json(url: &str, filename: &str) -> std::result::Result<Vec<Emote>, failure::Error> {
    let data = get_emote_json(url, filename)?;
    let mut v: Value = serde_json::from_str(&data)?;

    let mut emotes : Vec<Emote> = Vec::default();

    //if v[0]["data"]["channel"]["self"]["availableEmoteSets"].is_null() == false {
        for i in v[0]["data"]["channel"]["self"]["availableEmoteSets"].as_array_mut().unwrap() {
            for j in i["emotes"].as_array_mut().unwrap() {
                //writeln!(f, "{}", j["token"].to_string().trim_matches('"'))?;
                emotes.push(Emote { name: j["token"].to_string().trim_matches('"').to_owned(), data: None })
            }
        }
    //}

    Ok(emotes)
}

pub fn get_emote_json(url: &str, filename: &str) -> std::result::Result<String, failure::Error> {
    if Path::new(filename).exists() == false {
        let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(filename)
        .expect("Unable to open file");

        let mut easy = Easy::new();
        easy.url(url)?;
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