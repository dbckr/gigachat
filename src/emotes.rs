use curl::easy::Easy;
use eframe::{egui::{self}, epaint::{ColorImage, TextureHandle}};
use failure;
use glob::glob;
use image::DynamicImage;
use image::imageops::FilterType;
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
    pub data: Option<Vec<TextureHandle>>,
    pub loaded: bool,
    url: String,
    path: String,
    extension: Option<String>
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
        let Self { easy: _ } = self;
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
        let Self { easy: _ } = self;

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
                emotes.push(self.get_emote(name, id, imgurl, "generated/bttv/".to_owned(), Some(ext)));
            }
            for i in v["sharedEmotes"].as_array_mut().unwrap() {
                let name = i["code"].to_string().trim_matches('"').to_owned();
                let id = i["id"].to_string().trim_matches('"').to_owned();
                let ext = i["imageType"].to_string().trim_matches('"').to_owned();
                let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
                emotes.push(self.get_emote(name, id, imgurl, "generated/bttv/".to_owned(), Some(ext)));
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
                emotes.push(self.get_emote(name, id, imgurl, "generated/ffz/".to_owned(), None));
            }
        } else if v[0].is_null() == false {
            for i in v.as_array_mut().unwrap() {
                if i["code"].is_null() == false {
                    let name = i["code"].to_string().trim_matches('"').to_owned();
                    let id = i["id"].to_string().trim_matches('"').to_owned();
                    let ext = i["imageType"].to_string().trim_matches('"').to_owned();
                    let imgurl = format!("https://cdn.betterttv.net/emote/{}/3x", &id);
                    emotes.push(self.get_emote(name, id, imgurl, "generated/bttv/".to_owned(), Some(ext)));
                } else if i["name"].is_null() == false {
                    // 7tv
                    //emotes.push(Emote { name: i["name"].to_string().trim_matches('"').to_owned(), data: None })
                    let name = i["name"].to_string().trim_matches('"').to_owned();
                    let id = i["id"].to_string().trim_matches('"').to_owned();
                    let extension = i["mime"].to_string().trim_matches('"').replace("image/", "");
                    let x = i["urls"].as_array().unwrap().last().unwrap().as_array().unwrap().last().unwrap();
                    let imgurl = x.as_str().unwrap();
                    emotes.push(self.get_emote(
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
        url: String,
        path: String,
        extension: Option<String>,
    ) -> Emote {
        Emote {
            name: name,
            id: id,
            data: None,
            loaded: false,
            url: url,
            path: path,
            extension,
        }
    }

    pub fn load_image(&mut self, ctx: &egui::Context, emote : &mut Emote) {
        emote.data = self.get_image_data(ctx, &emote.url, &emote.path, &emote.id, &emote.extension);
    }

    fn get_image_data(
        &mut self,
        ctx: &egui::Context,
        url: &str,
        path: &str,
        id: &str,
        extension: &Option<String>,
    ) -> Option<Vec<TextureHandle>> {
        let Self { easy } = self;

        //let filename = format!("{}.{}", id, extension);

        let mut inner = ||
         -> std::result::Result<Option<Vec<TextureHandle>>, failure::Error> {
            if path.len() > 0 {
                DirBuilder::new().recursive(true).create(path)?;
            }

            match glob(&format!("{}{}.*", path, id))?.last() {
                Some(x) => {
                    let filepath = x?.as_path().to_owned();
                    let mut file = File::open(filepath.to_owned()).unwrap();
                    let mut buf : Vec<u8> = Default::default();
                    file.read_to_end(&mut buf)?;
                    Ok(load_image(ctx, filepath.extension().unwrap().to_str().unwrap(), &buf))
                } 
                None => {
                    if let Some(ref ext) = extension {
                        let mut extension: Option<String> = Some(ext.to_owned());
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

                        
                        let mut f = OpenOptions::new()
                            .create_new(true)
                            .write(true)
                            .open(format!("{}{}.{}", path, id, ext))?;

                        f.write(&buffer)?;
                        Ok(load_image(ctx, &ext, &buffer))
                    }
                    else {
                        Ok(None)
                    }
                }
            }
        };

        match inner() {
            Ok(x) => x,
            Err(x) => None,
        }
    }
}

fn load_image(ctx: &egui::Context, extension: &str, buffer : &[u8]) -> Option<Vec<TextureHandle>> {
    match extension {
        "png" => {
            match image::load_from_memory(&buffer) {
                Ok(img) => {
                    Some([load_image_into_texture_handle(ctx, &img)].to_vec())
                },
                _ => None
            }
        },
        "gif" => {
            load_animated_gif(ctx, &buffer)
        },
        "webp" => {
            load_animated_webp(ctx, &buffer)
        },
        _ => None
    }
}

fn load_animated_gif(ctx: &egui::Context, buffer : &[u8]) -> Option<Vec<TextureHandle>> {
    let mut loaded_frames : Vec<TextureHandle> = Default::default();
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = decoder.read_info(buffer).unwrap();
    let width = decoder.width() as u32;
    let height = decoder.height() as u32;
    //println!("{} {}", width, height);
    let mut last_frameimg : Option<DynamicImage>;

    // Handle first frame
    if let Some(frame) = decoder.read_next_frame().unwrap() {
        //let frametime = frame.delay;
        let temp : Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>>
        = image::ImageBuffer::from_raw(width, height, frame.buffer.to_vec());
        if let Some(imgbuf) = temp {
            last_frameimg = Some(DynamicImage::from(imgbuf));
        }
        else {
            last_frameimg = None;
        }
    }
    else {
        last_frameimg = None;
    }

    let reusable_img = &mut last_frameimg;

    while let Some(frame) = decoder.read_next_frame().unwrap() {
        let imgbufopt : Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> 
            = image::ImageBuffer::from_raw(width, height, frame.buffer.to_vec());
        if let Some(imgbuf) = imgbufopt {
            let image = DynamicImage::from(imgbuf);
            let handle : TextureHandle;
            if let Some(last_img) = reusable_img {
                image::imageops::overlay(last_img, &image, 0, 0);
                handle = load_image_into_texture_handle(ctx, &last_img);
            }
            else {
                handle = load_image_into_texture_handle(ctx, &image);
            }
            loaded_frames.push(handle);
        }
    }
    if loaded_frames.len() > 0 {
        Some(loaded_frames)
    }
    else {
        None
    }
}

fn load_animated_webp(ctx: &egui::Context, buffer : &[u8]) -> Option<Vec<TextureHandle>> {
    let mut loaded_frames : Vec<TextureHandle> = Default::default();
    let decoder = webp_animation::Decoder::new(&buffer).unwrap();
    let (width, height) = decoder.dimensions();
    for frame in decoder.into_iter() {
        //let frametime = frame.timestamp();
        let imgbufopt : Option<image::ImageBuffer<image::Rgba<u8>, _>> = image::ImageBuffer::from_raw(width, height, frame.data().to_vec());
        if let Some(imgbuf) = imgbufopt {
            let handle = load_image_into_texture_handle(ctx, &DynamicImage::from(imgbuf));
            loaded_frames.push(handle);
        }
    }
    if loaded_frames.len() > 0 {
        Some(loaded_frames)
    }
    else {
        None
    }
}

fn load_image_into_texture_handle(ctx: &egui::Context, image: &image::DynamicImage) -> TextureHandle {
    let uid = rand::random::<u128>(); //TODO: hash the image to create uid
    //let resize_width = image.width() * (24 / image.height());
    //let image = image.resize(resize_width, 24, FilterType::Lanczos3);
    let image = image.resize(24, 24, FilterType::Lanczos3);
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    let cimg = ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice()
    );
    ctx.load_texture(uid.to_string(), cimg)
  }