//#[allow(non_snake_case)]

#[cfg(test)]
mod test {
  use tracing_subscriber::{Registry, Layer, prelude::__tracing_subscriber_SubscriberExt};
  use std::{ops::Range, path::PathBuf};
  use curl::easy::Easy;
  use itertools::Itertools;
  use regex::Regex;
  use crate::{emotes::fetch, provider::dgg};
  use tracing_test::traced_test;
  use tracing_unwrap::{OptionExt};

  #[test]
  fn cursor_pos_test() {
    let msg = "GIG";
    let cursor_position = 3;

    let word = msg.split_whitespace()
      .map(move |s| (s.as_ptr() as usize - msg.as_ptr() as usize, s))
      .filter_map(|p| { println!("{}", p.0); if p.0 <= cursor_position && cursor_position <= p.0 + p.1.len() { Some((p.0, p.1)) } else { None } })
      .next().unwrap();

    println!("{:?}", word);
  }

  #[test]
  fn test() {
    let context : egui::Context = Default::default();
    let verifier = dgg::begin_authenticate(&context);
    println!("{}", verifier);
  }

  #[test]
  fn test2() {
    let css_path = "dgg-emotes.css";
    let css = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", Some(css_path), None, true).expect("failed to download emote css");
    let loader = dgg::CSSLoader::default();
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
    let css_path = "dgg-emotes.css";
    let css = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", Some(css_path), None, true).expect("failed to download emote css");

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
    let console = tracing_subscriber::fmt::layer()
    .with_line_number(true)
    .boxed();

    let subscriber = Registry::default().with(console);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default tracing subscriber");

    let buf = crate::emotes::imaging::load_file_into_buffer("7tv/60420a8b77137b000de9e66e.gif").unwrap_or_log();
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 60);
  }

  #[test]
  fn cummies() {
    let buf = crate::emotes::imaging::load_file_into_buffer("7tv/6129ca7da4d049e179751fe5.webp").unwrap_or_log();
    let frames = crate::emotes::imaging::load_animated_webp(&buf);
    assert_eq!(frames.unwrap().len(), 9);
  }

  #[test]
  fn gigachad() {
    let buf = crate::emotes::imaging::load_file_into_buffer("bttv/609431bc39b5010444d0cbdc.gif").unwrap_or_log();
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 198);
  }

  #[test]
  fn pokismash() {
    let buf = crate::emotes::imaging::load_file_into_buffer("bttv/5f0901cba2ac620530368579.gif").unwrap_or_log();
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 25);
  }

  #[test]
  fn peepoLeave() {
    let buf = crate::emotes::imaging::load_file_into_buffer("7tv/60b056f5b254a5e16b929707.webp").unwrap_or_log();
    let frames = crate::emotes::imaging::load_animated_webp(&buf);
    assert_eq!(frames.unwrap().len(), 11);
  }

  #[test]
  fn peepoLeave2() {
    let buf = crate::emotes::imaging::load_file_into_buffer("bttv/5d324913ff6ed36801311fd2.gif").unwrap_or_log();
    let frames = crate::emotes::imaging::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 35);
  }

  #[test]
  #[traced_test]
  fn load_emote() {
    let mut easy = Easy::new();
    let img = crate::emotes::imaging::get_image_data("KEKW", "https://cdn.betterttv.net/emote/5edcd164924aa35e32a73456/3x", PathBuf::new().join("bttv/"), "5edcd164924aa35e32a73456", &Some("gif".to_owned()), &mut easy, None);
    assert!(img.is_some());

    logs_assert(|lines: &[&str]| {
      lines.iter().for_each(|f| println!("{}", f));
      Ok(())
    });
  }

  #[test]
  fn estimate_test() {
    let x = estimate_message_test_helper(600., "⠄⠄⠄⠄⠄⠄⠄⢀⣠⣶⣾⣿⣶⣦⣤⣀⠄⢀⣀⣤⣤⣤⣤⣄⠄⠄⠄⠄⠄⠄ ⠄⠄⠄⠄⠄⢀⣴⣿⣿⣿⡿⠿⠿⠿⠿⢿⣷⡹⣿⣿⣿⣿⣿⣿⣷⠄⠄⠄⠄⠄ ⠄⠄⠄⠄⠄⣾⣿⣿⣿⣯⣵⣾⣿⣿⡶⠦⠭⢁⠩⢭⣭⣵⣶⣶⡬⣄⣀⡀⠄⠄ ⠄⠄⠄⡀⠘⠻⣿⣿⣿⣿⡿⠟⠩⠶⠚⠻⠟⠳⢶⣮⢫⣥⠶⠒⠒⠒⠒⠆⠐⠒ ⠄⢠⣾⢇⣿⣿⣶⣦⢠⠰⡕⢤⠆⠄⠰⢠⢠⠄⠰⢠⠠⠄⡀⠄⢊⢯⠄⡅⠂⠄ ⢠⣿⣿⣿⣿⣿⣿⣿⣏⠘⢼⠬⠆⠄⢘⠨⢐⠄⢘⠈⣼⡄⠄⠄⡢⡲⠄⠂⠠⠄ ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣷⣥⣀⡁⠄⠘⠘⠘⢀⣠⣾⣿⢿⣦⣁⠙⠃⠄⠃⠐⣀ ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣋⣵⣾⣿⣿⣿⣿⣦⣀⣶⣾⣿⣿⡉⠉⠉ ⣿⣿⣿⣿⣿⣿⣿⠟⣫⣥⣬⣭⣛⠿⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡆⠄ ⣿⣿⣿⣿⣿⣿⣿⠸⣿⣏⣙⠿⣿⣿⣶⣦⣍⣙⠿⠿⠿⠿⠿⠿⠿⠿⣛⣩⣶⠄ ⣛⣛⣛⠿⠿⣿⣿⣿⣮⣙⠿⢿⣶⣶⣭⣭⣛⣛⣛⣛⠛⠛⠻⣛⣛⣛⣛⣋⠁⢀ ⣿⣿⣿⣿⣿⣶⣬⢙⡻⠿⠿⣷⣤⣝⣛⣛⣛⣛⣛⣛⣛⣛⠛⠛⣛⣛⠛⣡⣴⣿ ⣛⣛⠛⠛⠛⣛⡑⡿⢻⢻⠲⢆⢹⣿⣿⣿⣿⣿⣿⠿⠿⠟⡴⢻⢋⠻⣟⠈⠿⠿ ⣿⡿⡿⣿⢷⢤⠄⡔⡘⣃⢃⢰⡦⡤⡤⢤⢤⢤⠒⠞⠳⢸⠃⡆⢸⠄⠟⠸⠛⢿ ⡟⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠁⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⠄⢸".to_owned(), true);
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
    let x = estimate_message_test_helper(300., "test".to_owned(), true);
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
    let x = estimate_message_test_helper(300., str.to_owned(), true);
    let expected_ranges : [Range<usize>; 5] = [(0..32),(32..88),(88..142),(142..196),(196..223)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  #[test]
  fn estimate_test_4() {
    let str = "This is a long sentence that is intended to test that text wraps over to a new line in an appropiate fashion in the user interface.".to_owned();
    let x = estimate_message_test_helper(300., str.to_owned(), true);
    let expected_ranges : [Range<usize>; 4] = [(0..24),(24..70),(70..112),(112..131)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  #[test]
  fn estimate_test_5() {
    let str = "This is a long sentence that is intended to test that text wraps over to a new line in an appropiate fashion in the user interface.".to_owned();
    let x = estimate_message_test_helper(300., str.to_owned(), false);
    let expected_ranges : [Range<usize>; 4] = [(0..32),(32..75),(75..116),(116..131)];
    let mut expected_iter = expected_ranges.iter();
    for (range, y, string) in x {
      assert_eq!(&range, expected_iter.next().unwrap());
      println!("{:<6}{:<10}{}", y, format!("{:?}", range), string);
    }
  }

  #[cfg(test)]
  fn estimate_message_test_helper(width: f32, message: String, show_timestamp: bool) -> Vec<(Range<usize>, f32, String)> {
    use std::collections::HashMap;
    use chrono::Utc;
    use egui::{LayerId, Order, Pos2, Rect, Id};
    use crate::{provider::{UserProfile, ChatMessage, MessageType}, ui::{chat::EmoteFrame, chat_estimate::{TextRange, get_chat_msg_size}, load_font}};

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
      username: "moonmoon".to_owned(), 
      timestamp: Utc::now(), 
      message: message.to_owned(), 
      profile: UserProfile {
        badges: None, //Some(vec!["sub".to_owned()]),
        display_name: None,
        color: Some((0, 0, 0)),
      }, 
      combo_data: None,
      is_removed: None,
      msg_type: MessageType::Chat };
    let emotes : HashMap<String, EmoteFrame> = Default::default();
    let badges : HashMap<String, EmoteFrame> = Default::default();
    let ui_width = ui.available_width();
    
    let x = get_chat_msg_size(&mut ui, ui_width, &msg, &emotes, Some(&badges), false, show_timestamp);

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