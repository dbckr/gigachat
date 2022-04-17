use curl::easy::Easy;
use failure;
use glob::glob;
use image::DynamicImage;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{BufRead, BufReader, Write, Read};
use std::path::Path;
use std::str;

//#[derive(Clone)]
pub struct Emote {
    pub name: String,
    pub id: String,
    pub data: Option<Vec<DynamicImage>>,
}

pub struct EmoteLoader {
    easy: Easy,
}

impl Default for EmoteLoader {
    fn default() -> Self {
        Self { easy: Easy::new() }
    }
}

impl EmoteLoader {
    pub fn load_channel_emotes(
        &mut self,
        channel_id: &String,
    ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
        let Self { easy } = self;
        let ffz_url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);
        let ffz_emotes = self.process_emote_json(
            &ffz_url,
            &format!("generated/ffz-channel-json-{}", channel_id),
        )?;
        let bttv_url = format!(
            "https://api.betterttv.net/3/cached/users/twitch/{}",
            channel_id
        );
        let bttv_emotes = self.process_emote_json(
            &bttv_url,
            &format!("generated/bttv-channel-json-{}", channel_id),
        )?;
        let seventv_url = format!("https://api.7tv.app/v2/users/{}/emotes", channel_id);
        let seventv_emotes = self.process_emote_json(
            &seventv_url,
            &format!("generated/7tv-channel-json-{}", channel_id),
        )?;

        let mut result: HashMap<String, Emote> = HashMap::new();
        for emote in ffz_emotes {
            result.insert(emote.name.to_owned(), emote);
        }
        for emote in bttv_emotes {
            result.insert(emote.name.to_owned(), emote);
        }
        for emote in seventv_emotes {
            result.insert(emote.name.to_owned(), emote);
        }
        Ok(result)
    }

    pub fn load_global_emotes(
        &mut self,
    ) -> std::result::Result<HashMap<String, Emote>, failure::Error> {
        let bttv_emotes = self.process_emote_json(
            "https://api.betterttv.net/3/cached/emotes/global",
            "generated/bttv-global-json",
        )?;
        let seventv_emotes = self.process_emote_json(
            "https://api.7tv.app/v2/emotes/global",
            "generated/7tv-global-json",
        )?;

        let mut result: HashMap<String, Emote> = HashMap::new();

        let unknown_image = image::open("img/DEFAULT.png").unwrap();
        result.insert(
            "_".to_owned(),
            Emote {
                name: "DEFAULT".to_owned(),
                id: "-1".to_owned(),
                data: Some([unknown_image].to_vec()),
            },
        );

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
        Ok(BufReader::new(file)
            .lines()
            .map(|l| l.expect("Could not parse line"))
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

    pub fn process_emote_json(
        &mut self,
        url: &str,
        filename: &str,
    ) -> std::result::Result<Vec<Emote>, failure::Error> {
        let Self { easy } = self;

        println!("processing emote json {}", filename);
        let data = self.get_emote_json(url, filename)?;
        let mut v: Value = serde_json::from_str(&data)?;

        let mut emotes: Vec<Emote> = Vec::default();

        if v["channelEmotes"].is_null() == false {
            // BTTV cache api
            for i in v["channelEmotes"].as_array_mut().unwrap() {
                let name = i["code"].to_string().trim_matches('"').to_owned();
                let id = i["id"].to_string().trim_matches('"').to_owned();
                let ext = i["imageType"].to_string().trim_matches('"').to_owned();
                let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
                emotes.push(self.get_emote(name, id, &imgurl, "generated/bttv/", Some(ext)));
            }
            for i in v["sharedEmotes"].as_array_mut().unwrap() {
                let name = i["code"].to_string().trim_matches('"').to_owned();
                let id = i["id"].to_string().trim_matches('"').to_owned();
                let ext = i["imageType"].to_string().trim_matches('"').to_owned();
                let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
                emotes.push(self.get_emote(name, id, &imgurl, "generated/bttv/", Some(ext)));
            }
        } else if v["emotes"].is_null() == false {
            // BTTV channel name based API
            //e.g. get_emote_json("https://api.betterttv.net/2/channels/jormh", "bttv-jormh-json")?;
            for i in v["emotes"].as_array_mut().unwrap() {
                //emotes.push(Emote { name: i["code"].to_string().trim_matches('"').to_owned(), data: None })
            }
        } else if v["room"].is_null() == false {
            // FFZ
            let setid = v["room"]["set"].to_string();
            for i in v["sets"][&setid]["emoticons"].as_array_mut().unwrap() {
                //TODO: Try to get i["urls"]["4"] then i["urls"]["2"] then i["urls"]["1"] in that order of precedence
                let imgurl = format!("https:{}", i["urls"]["4"].to_string().trim_matches('"'));
                let name = i["name"].to_string().trim_matches('"').to_owned();
                let id = i["id"].to_string().trim_matches('"').to_owned();
                emotes.push(self.get_emote(name, id, &imgurl, "generated/ffz/", None));
            }
        } else if v[0].is_null() == false {
            for i in v.as_array_mut().unwrap() {
                if i["code"].is_null() == false {
                    let name = i["code"].to_string().trim_matches('"').to_owned();
                    let id = i["id"].to_string().trim_matches('"').to_owned();
                    let ext = i["imageType"].to_string().trim_matches('"').to_owned();
                    let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
                    emotes.push(self.get_emote(name, id, &imgurl, "generated/bttv/", Some(ext)));
                } else if i["name"].is_null() == false {
                    // 7tv
                    //emotes.push(Emote { name: i["name"].to_string().trim_matches('"').to_owned(), data: None })
                    let name = i["name"].to_string().trim_matches('"').to_owned();
                    let id = i["id"].to_string().trim_matches('"').to_owned();
                    let extension = i["mime"]
                        .to_string()
                        .trim_matches('"')
                        .replace("image/", "");
                    let x = i["urls"]
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap();
                    let imgurl = x.as_str().unwrap();
                    emotes.push(self.get_emote(
                        name,
                        id,
                        imgurl.trim_matches('"'),
                        "generated/7tv/",
                        Some(extension),
                    ));
                }
            }
        }

        Ok(emotes)
    }

    pub fn process_twitch_json(
        &mut self,
        url: &str,
        filename: &str,
    ) -> std::result::Result<Vec<Emote>, failure::Error> {
        let data = self.get_emote_json(url, filename)?;
        let mut v: Value = serde_json::from_str(&data)?;

        let mut emotes: Vec<Emote> = Vec::default();

        //if v[0]["data"]["channel"]["self"]["availableEmoteSets"].is_null() == false {
        for i in v[0]["data"]["channel"]["self"]["availableEmoteSets"]
            .as_array_mut()
            .unwrap()
        {
            for j in i["emotes"].as_array_mut().unwrap() {
                //writeln!(f, "{}", j["token"].to_string().trim_matches('"'))?;
                //emotes.push(get_emote( j["token"].to_string().trim_matches('"').to_owned(), data: None })
            }
        }
        //}

        Ok(emotes)
    }

    pub fn get_emote_json(
        &mut self,
        url: &str,
        filename: &str,
    ) -> std::result::Result<String, failure::Error> {
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
        for line in BufReader::new(file)
            .lines()
            .filter_map(|result| result.ok())
        {
            result.push_str(&line);
        }
        Ok(result)
    }

    pub fn get_emote(
        &mut self,
        name: String,
        id: String,
        url: &str,
        path: &str,
        extension: Option<String>,
    ) -> Emote {
        Emote {
            name: name,
            data: self.get_image_data(url, path, &id, extension),
            id: id,
        }
    }

    pub fn get_image_data(
        &mut self,
        url: &str,
        path: &str,
        filename: &str,
        extension: Option<String>,
    ) -> Option<Vec<DynamicImage>> {
        let Self { easy } = self;

        let mut inner = |url,
                         filename: &str|
         -> std::result::Result<Option<Vec<DynamicImage>>, failure::Error> {
            if path.len() > 0 {
                DirBuilder::new().recursive(true).create(path)?;
            }

            match glob(&format!("{}{}.*", path, filename))?.last() {
                Some(x) => {
                    let filepath = x?.as_path().to_owned();
                    let mut file = File::open(filepath.to_owned()).unwrap();
                    let mut buf : Vec<u8> = Default::default();
                    file.read_to_end(&mut buf)?;
                    Ok(load_image(filepath.extension().unwrap().to_str().unwrap(), &buf))
                } 
                None => {
                    let mut extension: Option<String> = extension;
                    let mut buffer: Vec<u8> = Default::default();

                    easy.url(url)?;
                    let mut transfer = easy.transfer();
                    if extension == None {
                        transfer.header_function(|data| {
                        let result = str::from_utf8(data);
                        if let Ok(header) = result && (header.contains("content-disposition") || header.contains("content-type")) {
                            //TODO: extract extension using regex
                            if header.to_lowercase().contains(".png") || header.to_lowercase().ends_with("png") {
                                extension = Some("png".to_owned());
                            }
                            else if header.to_lowercase().contains(".gif") || header.to_lowercase().ends_with("gif") {
                                extension = Some("gif".to_owned());
                            }
                            else if header.to_lowercase().contains(".webp") || header.to_lowercase().ends_with("webp") {
                                extension = Some("webp".to_owned());
                            }
                        }
                        true
                    })?;
                    }
                    transfer.write_function(|data| {
                        //f.write_all(data).expect("Failed to write to file");
                        for byte in data {
                            buffer.push(byte.to_owned());
                        }
                        Ok(data.len())
                    })?;
                    transfer.perform()?;
                    drop(transfer);

                    if let Some(ref ext) = extension {
                        let mut f = OpenOptions::new()
                            .create_new(true)
                            .write(true)
                            .open(format!("{}{}.{}", path, filename, ext))?;

                        f.write(&buffer)?;
                        Ok(load_image(&ext, &buffer))
                    }
                    else {
                        Ok(None)
                    }
                }
            }
        };

        match inner(url, filename) {
            Ok(x) => x,
            Err(x) => None,
        }
    }
}

fn load_image(extension: &str, buffer : &[u8]) -> Option<Vec<DynamicImage>> {
    match extension {
        "png" => {
            match image::load_from_memory(&buffer) {
                Ok(img) => Some([img].to_vec()),
                _ => None
            }
        },
        "gif" => {
            load_animated_gif(&buffer)
        },
        _ => None
    }
}

fn load_animated_gif(buffer : &[u8]) -> Option<Vec<DynamicImage>> {
    let mut loaded_frames : Vec<DynamicImage> = Default::default();
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = decoder.read_info(buffer).unwrap();
    let width = decoder.width() as u32;
    let height = decoder.height() as u32;
    println!("{} {}", width, height);
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        // Process every frame
        let imgbufopt : Option<image::ImageBuffer<image::Rgba<u8>, _>> = image::ImageBuffer::from_raw(width, height, frame.buffer.to_vec());
        if let Some(imgbuf) = imgbufopt {
            loaded_frames.push(DynamicImage::from(imgbuf));
        }
        /*match image::load_from_memory(&frame.buffer) {
            Ok(img) => loaded_frames.push(img),
            Err(x) => { println!("{:?}", x); () }
        };*/
    }
    if loaded_frames.len() > 0 {
        Some(loaded_frames)
    }
    else {
        None
    }
}