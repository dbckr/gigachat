#[allow(non_snake_case)]

#[cfg(test)]
mod test {
    use std::{path::PathBuf, collections::HashMap, ops::Range};

    use chrono::Utc;
    use curl::easy::Easy;
    use egui::{LayerId, Id, Order, Rect, Pos2};
    use itertools::Itertools;
    use regex::Regex;

    use crate::{ui::{chat_estimate::{get_chat_msg_size, TextRange}, chat::EmoteFrame, load_font}, provider::{ChatMessage, UserProfile, dgg}, emotes::fetch};

  #[test]
  fn test() {
    let context : egui::Context = Default::default();
    let verifier = dgg::begin_authenticate(&context);
    println!("{}", verifier);
  }

  #[test]
  fn test2() {
    let css_path = "cache/dgg-emotes.css";
    let css = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", Some(css_path), None).expect("failed to download emote css");
    let loader = dgg::CSSLoader::new();
    let data = loader.get_css_anim_data(&css);

    let closure = |prefix : &str| {
      let result = data.get(prefix);
      println!("{:?}", result);
    };
    
    closure("RaveDoge");
    closure("WooYeah");
    closure("WOOF");
    closure("pepeSteer");
    closure("OOOO");
  }

  #[test]
  fn test3() {
    let css_path = "cache/dgg-emotes.css";
    let css = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", Some(css_path), None).expect("failed to download emote css");
    let loader = dgg::CSSLoader::new();

    //let regex = Regex::new(r"\.emote\.([^:\-\s]*?)\s\{.*? width: (\d+?)px;.*?animation: (?:[^\s]*?) (.*?);").unwrap();
    let regex = Regex::new(r"(?s)\.emote\.([^:\-\s]*?)\s?\{[^\}]*? width: (\d+?)px;[^\}]*?animation: (?:[^\s]*?) ([^\}]*?;)").unwrap();
    let caps = regex.captures_iter(&css);

    for cap in caps {
      println!("{}", cap.iter().skip(1).map(|x| format!("{:?}", x.unwrap().as_str())).join(", "));
    }
  }

  #[test]
  fn test4() {
    let x = format!("MSG {{\"data\":\"{}\"}}", "message");
    println!("{}", x);
  }

  #[test]
  fn gachihyper() {
    let buf = crate::emotes::imaging::load_file_into_buffer("cache/7tv/60420a8b77137b000de9e66e.gif");
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 60);
  }

  #[test]
  fn cummies() {
    let buf = crate::emotes::imaging::load_file_into_buffer("cache/7tv/6129ca7da4d049e179751fe5.webp");
    let frames = crate::emotes::imaging::load_animated_webp(&buf);
    assert_eq!(frames.unwrap().len(), 9);
  }

  #[test]
  fn gigachad() {
    let buf = crate::emotes::imaging::load_file_into_buffer("cache/bttv/609431bc39b5010444d0cbdc.gif");
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 198);
  }

  #[test]
  fn pokismash() {
    let buf = crate::emotes::imaging::load_file_into_buffer("cache/bttv/5f0901cba2ac620530368579.gif");
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 25);
  }

  #[test]
  fn peepoLeave() {
    let buf = crate::emotes::imaging::load_file_into_buffer("cache/7tv/60b056f5b254a5e16b929707.webp");
    let frames = crate::emotes::imaging::load_animated_webp(&buf);
    assert_eq!(frames.unwrap().len(), 35);
  }

  #[test]
  fn peepoLeave2() {
    let buf = crate::emotes::imaging::load_file_into_buffer("cache/bttv/5d324913ff6ed36801311fd2.gif");
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 35);
  }

  #[test]
  fn load_emote() {
    let mut easy = Easy::new();
    let img = crate::emotes::imaging::get_image_data("https://cdn.betterttv.net/emote/5edcd164924aa35e32a73456/3x", PathBuf::new().join("cache/bttv/"), "5edcd164924aa35e32a73456", &Some("gif".to_owned()), &mut easy, None);
    assert!(img.is_some());
  }

  #[test]
  fn estimate_test() {
    let x = estimate_message_test_helper(600., "⠄⠄⠄⠄⠄⠄⠄⢀⣠⣶⣾⣿⣶⣦⣤⣀⠄⢀⣀⣤⣤⣤⣤⣄⠄⠄⠄⠄⠄⠄ ⠄⠄⠄⠄⠄⢀⣴⣿⣿⣿⡿⠿⠿⠿⠿⢿⣷⡹⣿⣿⣿⣿⣿⣿⣷⠄⠄⠄⠄⠄ ⠄⠄⠄⠄⠄⣾⣿⣿⣿⣯⣵⣾⣿⣿⡶⠦⠭⢁⠩⢭⣭⣵⣶⣶⡬⣄⣀⡀⠄⠄ ⠄⠄⠄⡀⠘⠻⣿⣿⣿⣿⡿⠟⠩⠶⠚⠻⠟⠳⢶⣮⢫⣥⠶⠒⠒⠒⠒⠆⠐⠒ ⠄⢠⣾⢇⣿⣿⣶⣦⢠⠰⡕⢤⠆⠄⠰⢠⢠⠄⠰⢠⠠⠄⡀⠄⢊⢯⠄⡅⠂⠄ ⢠⣿⣿⣿⣿⣿⣿⣿⣏⠘⢼⠬⠆⠄⢘⠨⢐⠄⢘⠈⣼⡄⠄⠄⡢⡲⠄⠂⠠⠄ ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣷⣥⣀⡁⠄⠘⠘⠘⢀⣠⣾⣿⢿⣦⣁⠙⠃⠄⠃⠐⣀ ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣋⣵⣾⣿⣿⣿⣿⣦⣀⣶⣾⣿⣿⡉⠉⠉ ⣿⣿⣿⣿⣿⣿⣿⠟⣫⣥⣬⣭⣛⠿⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡆⠄ ⣿⣿⣿⣿⣿⣿⣿⠸⣿⣏⣙⠿⣿⣿⣶⣦⣍⣙⠿⠿⠿⠿⠿⠿⠿⠿⣛⣩⣶⠄ ⣛⣛⣛⠿⠿⣿⣿⣿⣮⣙⠿⢿⣶⣶⣭⣭⣛⣛⣛⣛⠛⠛⠻⣛⣛⣛⣛⣋⠁⢀ ⣿⣿⣿⣿⣿⣶⣬⢙⡻⠿⠿⣷⣤⣝⣛⣛⣛⣛⣛⣛⣛⣛⠛⠛⣛⣛⠛⣡⣴⣿ ⣛⣛⠛⠛⠛⣛⡑⡿⢻⢻⠲⢆⢹⣿⣿⣿⣿⣿⣿⠿⠿⠟⡴⢻⢋⠻⣟⠈⠿⠿ ⣿⡿⡿⣿⢷⢤⠄⡔⡘⣃⢃⢰⡦⡤⡤⢤⢤⢤⠒⠞⠳⢸⠃⡆⢸⠄⠟⠸⠛⢿ ⡟⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠁⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⢸".to_owned());
    let expected_ranges : [Range<usize>; 16] = [(0..0),(0..30),(31..61),(62..92),(93..123),(124..154),(155..185),(186..216),
      (217..247),(248..278),(279..309),(310..340),(341..371),(372..402),(403..433),(434..464)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  
  #[test]
  fn estimate_test_2() {
    let x = estimate_message_test_helper(300., "test".to_owned());
    let expected_ranges : [Range<usize>; 1] = [(0..4)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  #[test]
  fn estimate_test_3() {
    let str = "kslajflksadjflksdjlfkjsdlakfjlkasjdflsdjafkljsdalfjksdlakfjsdlakfjldsjflsdakjflksdjflkjsdalfkjasldkfjlsadkjflsakdjflkasjdlfkjasdklfjlsdakfjklsdajflsdakjflsdjaflksdjflsdkajflsakdjflksadjflksdajflksjdlafkjsdklafjlsadkfjsdlfas".to_owned();
    let x = estimate_message_test_helper(300., str.to_owned());
    let expected_ranges : [Range<usize>; 5] = [(0..32),(32..88),(88..142),(142..196),(196..223)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  #[test]
  fn estimate_test_4() {
    let str = "This is a long sentence intended to test that text wraps over to a new line in an appropiate fashion in the user interface.".to_owned();
    let x = estimate_message_test_helper(300., str.to_owned());
    let expected_ranges : [Range<usize>; 4] = [(0..24),(24..67),(67..108),(108..123)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  fn estimate_message_test_helper(width: f32, message: String) -> Vec<(Range<usize>, f32, String)> {
    let context : egui::Context = Default::default();
    context.set_fonts(load_font());
    context.begin_frame(Default::default());
    let id = Id::new(123);
    let layer = LayerId::new(Order::Debug, id);
    let rect = Rect { min: Pos2 { x: 0., y: 0. }, max: Pos2 { x: width, y: 400. } };
    let mut ui = egui::Ui::new(context, layer, id, rect, rect);
    let msg = ChatMessage { 
      provider: crate::provider::ProviderName::Twitch, 
      channel: "xqcow".to_owned(), 
      username: "xqcL".to_owned(), 
      timestamp: Utc::now(), 
      message: message.to_owned(), 
      profile: UserProfile {
        badges: None,
        display_name: None,
        color: Some((0, 0, 0)),
      }, 
      combo_data: None };
    let emotes : HashMap<String, EmoteFrame> = Default::default();
    let badges : HashMap<String, EmoteFrame> = Default::default();
    let x = get_chat_msg_size(&mut ui, &msg, &emotes, Some(&badges));
    
    x.0.iter().map(|item| {
      let rng = match &item.1 {
        TextRange::Range { range } => range,
        _ => panic!("unexpected")
      };
      (
        rng.to_owned(), 
        item.0,
        //message[rng.start..rng.end].to_owned() //TODO: make estimator return slice ix instead of char ix (performance?)
        message.char_indices().map(|(_i, x)| x).skip(rng.start).take(rng.end - rng.start).collect::<String>()
      )
    }).collect_vec()
  }
}
