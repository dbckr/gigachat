#[allow(non_snake_case)]

#[cfg(test)]
mod test {
    use regex::Regex;

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
  fn peepoLeave() {
    let buf = crate::emotes::load_file_into_buffer("generated/7tv/60b056f5b254a5e16b929707.webp");
    let frames = crate::emotes::load_animated_webp(&buf);
    assert_eq!(frames.unwrap().len(), 35);
  }

  #[test]
  fn peepoLeave2() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/5d324913ff6ed36801311fd2.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 35);
  }

  #[test]
  fn test222() {
    let ix : usize = 1;
    let x = ix.saturating_sub(2);
    assert_eq!(x, 0);
  }
}
