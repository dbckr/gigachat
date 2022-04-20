#[cfg(test)]
mod test {
  #[test]
  fn gachihyper() {
    let buf = crate::emotes::load_file_into_buffer("generated/7tv/60420a8b77137b000de9e66e.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 60);
  }

  #[test]
  fn gigachad() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/609431bc39b5010444d0cbdc.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 198);
  }

  #[test]
  fn pokismash() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/5f0901cba2ac620530368579.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 25);
  }

  #[test]
  fn pepeAim() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/5d0d7140ca4f4b50240ff6b4.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 61);
  }

  #[test]
  fn clap() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/55b6f480e66682f576dd94f5.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 2);
  }

  #[test]
  fn elnosabe() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/605ab0317493072efdeb3698.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 4);
  }

  #[test]
  fn omegalaughing() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/5b3fd6770f8f6c2547825a6f.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 12);
  }

  //60af09d9a564afa26e8f19ef
  #[test]
  fn peepoLeave() {
    let buf = crate::emotes::load_file_into_buffer("generated/7tv/60b056f5b254a5e16b929707.webp");
    let frames = crate::emotes::load_animated_webp(&buf);
    assert_eq!(frames.unwrap().len(), 35);
  }
}
