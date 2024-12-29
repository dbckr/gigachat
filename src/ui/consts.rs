/// Max length before manually splitting up a string without whitespace
pub const WORD_LENGTH_MAX : usize = 30;

/// Emotes in chat messages will be scaled to this height, relative to chat text font size
pub const EMOTE_SCALING : f32 = 1.6;
pub const BADGE_HEIGHT : f32 = 18.0;

/// Should be at least equal to ui.spacing().interact_size.y
pub const MIN_LINE_HEIGHT : f32 = 22.0;
pub const COMBO_LINE_HEIGHT : f32 = 42.0;

pub const DEFAULT_USER_COLOR : (u8,u8,u8) = (255,255,255);

pub const NEW_MESSAGES_PER_FRAME : usize = 50;