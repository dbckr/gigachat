use std::usize::MAX;

use eframe::epaint::Color32;

pub mod twitch;

pub fn convert_color_hex(hex_string: Option<&String>) -> (u8, u8, u8) {
  match hex_string {
    Some(hex_str) => { 
      if hex_str == "" {
        return (255,255,255)
      }
      match hex::decode(hex_str.trim_start_matches("#")) {
        Ok(val) => (val[0], val[1], val[2]),
        Err(_) => {
          println!("ERROR {}", hex_str);
          (255, 255, 255)
        }
      }
    },
    None => (255, 255, 255)
  }
}

pub fn convert_color(input : &(u8, u8, u8)) -> Color32 {
  // return white
  if input == &(255u8, 255u8, 255u8) {
    return Color32::WHITE;
  }

  // normalize brightness
  let target = 200;

  let min = |x, y| -> u8 {
    let z = x < y;
    match z {
      true => x,
      _ => y
    }
  };

  let tf = |x| -> (u8, u8) {
    if x < target {
      (target - x, 255 - x)
    }
    else {
      (0, 255 - x)
    }
  };

  let (r, g, b) = (input.0, input.1, input.2);

  let (r_diff, r_max_adj) = tf(r);
  let (g_diff, g_max_adj) = tf(g);
  let (b_diff, b_max_adj) = tf(b);

  let adj = ((r_diff as u16 + g_diff as u16 + b_diff as u16) / 3) as u8;

  let (rx, gx, bx) = (r + min(adj, r_max_adj), g + min(adj, g_max_adj), b + min(adj, b_max_adj));

  //println!("{} {} {}", rx, gx, bx);
  return Color32::from_rgb(rx, gx, bx);
}