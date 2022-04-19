#[cfg(test)]
mod test {
  #[test]
  /*fn gachihyper() {
    let buf = crate::emotes::load_file_into_buffer("generated/7tv/60420a8b77137b000de9e66e.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 59);
  }*/

  #[test]
  fn gigachad() {
    let buf = crate::emotes::load_file_into_buffer("generated/bttv/609431bc39b5010444d0cbdc.gif");
    let frames = crate::emotes::load_animated_gif(&buf);
    assert_eq!(frames.unwrap().len(), 197);
  }
}
