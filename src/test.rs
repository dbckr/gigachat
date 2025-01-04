#[allow(clippy::all, dead_code, non_snake_case)]

#[cfg(test)]
mod test {
  use tracing_subscriber::{Registry, Layer, prelude::__tracing_subscriber_SubscriberExt};
  //use tracing_test::traced_test;
  use tracing_unwrap::OptionExt;
  use crate::provider::dgg;

  fn test() {
    let verifier = dgg::begin_authenticate();
    println!("{}", verifier);
  }

  /*#[test]
  fn test_json() {
    let json = fetch::get_json_from_url(format!("{}/flairs/flairs.json", "https://cdn.destiny.gg/2.42.0").as_str(), Some("dgg-flairs.json"), None, true).unwrap();
    let emotes = serde_json::from_str::<Vec<DggFlair>>(&json);
    println!("{:?}", emotes);
  }*/

  #[tokio::test]
  async fn test2() {
    let css_path = "dgg-emotes.css";
    let client = reqwest::Client::new();
    let css = crate::emotes::fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", Some(css_path), None, &client, true).await.expect("failed to download emote css");
    let loader = dgg::CSSLoader::default();
    let data = loader.get_css_anim_data(&css);

    let closure = |prefix : &str| {
      let result = data.get(prefix);
      println!("{:?}", result);
    };
    
    // closure("RaveDoge");
    // closure("WooYeah");
    // closure("WOOF");
    // closure("pepeSteer");
    // closure("OOOO");
    closure("WEOW");
    closure("Chatting");
    closure("KEIKAKU");

    let result = data.get("Chatting");
    assert!(result.is_some());
    assert_eq!(result.unwrap().width, 32);
    assert_eq!(result.unwrap().steps, 4);
  }

  /*#[test]
  fn test3() {
    let css_path = "dgg-emotes.css";
    let css = fetch::get_json_from_url("https://cdn.destiny.gg/2.42.0/emotes/emotes.css", Some(css_path), None, true).expect("failed to download emote css");

    //let regex = Regex::new(r"\.emote\.([^:\-\s]*?)\s\{.*? width: (\d+?)px;.*?animation: (?:[^\s]*?) (.*?);").unwrap();
    let regex = Regex::new(r"(?s)\.emote\.([^:\-\s]*?)\s?\{[^\}]*? width: (\d+?)px;[^\}]*?animation: (?:[^\s]*?) ([^\}]*?;)").unwrap();
    let caps = regex.captures_iter(&css);

    for cap in caps {
      println!("{}", cap.iter().skip(1).map(|x| format!("{:?}", x.unwrap().as_str())).join(", "));
    }
  }*/

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

  /*#[test]
  #[traced_test]
  fn load_emote() {
    let mut easy = reqwest::Client::new();
    let img = crate::emotes::imaging::get_image_data("KEKW", "https://cdn.betterttv.net/emote/5edcd164924aa35e32a73456/3x", PathBuf::new().join("bttv/"), "5edcd164924aa35e32a73456", &Some("gif".to_owned()), &mut easy, None);
    assert!(img.is_some());

    logs_assert(|lines: &[&str]| {
      lines.iter().for_each(|f| println!("{}", f));
      Ok(())
    });
  }*/

}