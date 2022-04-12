use curl::easy::{Easy};
use serde_json::Value;
use std::io::{Write, BufReader, BufRead};
use std::fs::{OpenOptions, File};
use std::str;
use failure;
use std::path::Path;

pub fn is_only_emotes(msg: &String, emotes: &Vec<String>) -> bool {
    let parts = msg.split(" ");
    for i in parts {
        if emotes.contains(&i.to_owned()) == false {
            //println!("not an emote: {}", &i);
            return false;
        }
    }

    return true;
}

pub fn parse_emotes(msg: &String, emotes: &Vec<String>) -> Vec<String> {
    let parts = msg.split(" ");
    let mut v : Vec<String> = Vec::new();
    let mut combo : String = "".to_string();
    for i in parts {
        let part = &i.to_owned();
        if emotes.contains(part) {
            combo.push_str(part);
            combo.push_str(" ");
        }
        else if combo != "" {
            let fin = combo.trim_end().to_owned();
            if v.contains(&fin) == false {
                v.push(fin);
            }
            combo = "".to_string();
        }
    }

    if combo.trim_end() != "" {
        let fin = combo.trim_end().to_owned();
        if v.contains(&fin) == false {
            v.push(fin);
        }
    }

    //if v.len() > 0 {
    //    println!("{:?}", v);
    //}
    return v;
}

pub fn load_emotes() -> std::result::Result<Vec<String>, failure::Error> {

    let bttv_url = std::env::var("BTTV_CHANNEL_URL")?;
    let ffz_url = std::env::var("FFZ_CHANNEL_URL")?;
    let seventv_url = std::env::var("SEVENTV_CHANNEL_URL")?;

    get_emote_json("https://api.betterttv.net/3/cached/emotes/global", "generated/bttv-global-json")?;
    get_emote_json(&bttv_url, "generated/bttv-channel-json")?;
    get_emote_json(&ffz_url, "generated/ffz-channel-json")?;
    get_emote_json(&seventv_url, "generated/7tv-channel-json")?;
    get_emote_json("https://api.7tv.app/v2/emotes/global", "generated/7tv-global-json")?;

    {
        OpenOptions::new()
        .truncate(true)
        .write(true)
        .create(true)
        .open("generated/emotes")
        .expect("Unable to open file");
    }

    process_twitch_json("config/twitch-json-data")?;

    process_emote_json("generated/bttv-global-json")?;
    process_emote_json("generated/bttv-channel-json")?;
    process_emote_json("generated/ffz-channel-json")?;
    process_emote_list("config/custom")?;
    //process_emote_list("config/twitch-global")?;
    process_emote_json("generated/7tv-channel-json")?;
    process_emote_json("generated/7tv-global-json")?;
    process_emote_list("config/generic-emoji-list")?;
    process_emote_list("config/emoji-unicode")?;

    load_list_file("generated/emotes")
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

pub fn process_emote_json(filename: &str) -> std::result::Result<(), failure::Error> {
    println!("processing emote json {}", filename);
    let mut f = OpenOptions::new()
    .append(true)
    .create(true) // Optionally create the file if it doesn't already exist
    .open("generated/emotes")
    .expect("Unable to open file");

    let data = std::fs::read_to_string(&filename).expect("Unable to read file");
    let mut v: Value = serde_json::from_str(&data)?;

    if v["channelEmotes"].is_null() == false { // BTTV cache api
        for i in v["channelEmotes"].as_array_mut().unwrap() {
            writeln!(f, "{}", i["code"].to_string().trim_matches('"'))?;
        }
        for i in v["sharedEmotes"].as_array_mut().unwrap() {
            writeln!(f, "{}", i["code"].to_string().trim_matches('"'))?;
        }
    }
    else if v["emotes"].is_null() == false { // BTTV channel name based API
        //e.g. get_emote_json("https://api.betterttv.net/2/channels/jormh", "bttv-jormh-json")?;
        for i in v["emotes"].as_array_mut().unwrap() {
            writeln!(f, "{}", i["code"].to_string().trim_matches('"'))?;
        }
    }
    else if v["room"].is_null() == false { // FFZ
        let setid = v["room"]["set"].to_string();
        for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap() {
            writeln!(f, "{}", i["name"].to_string().trim_matches('"'))?;
        }
    }
    else if v[0].is_null() == false {
        for i in v.as_array_mut().unwrap() {
            if i["code"].is_null() == false {
                writeln!(f, "{}", i["code"].to_string().trim_matches('"'))?;
            }
            else {
                writeln!(f, "{}", i["name"].to_string().trim_matches('"'))?;
            }
        }
    }

    Ok(())
}

pub fn process_twitch_json(filename: &str) -> std::result::Result<(), failure::Error> {
    let mut f = OpenOptions::new()
    .append(true)
    .create(true) // Optionally create the file if it doesn't already exist
    .open("generated/emotes")
    .expect("Unable to open file");

    let data = std::fs::read_to_string(&filename).expect("Unable to read file");
    let mut v: Value = serde_json::from_str(&data)?;

    //if v[0]["data"]["channel"]["self"]["availableEmoteSets"].is_null() == false {
        for i in v[0]["data"]["channel"]["self"]["availableEmoteSets"].as_array_mut().unwrap() {
            for j in i["emotes"].as_array_mut().unwrap() {
                writeln!(f, "{}", j["token"].to_string().trim_matches('"'))?;
            }
        }
    //}

    Ok(())
}

pub fn get_emote_json(url: &str, filename: &str) -> std::result::Result<(), failure::Error> {
    if Path::new(filename).exists() == false {
        println!("fetching emote json {}", url);
        let mut f = OpenOptions::new()
        .append(true)
        .create(true) // Optionally create the file if it doesn't already exist
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

    Ok(())
}