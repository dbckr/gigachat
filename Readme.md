# Features

All the usual basic features you would expect in a twitch chat app: channel tabs, all the emotes (twitch/ffz/bttv/7tv/animated/zero-width/etc), emote selector, etc...

# Todo

- Own twitch badges inaccurate when swapping between channels
- Display a "button" to jump back to bottom whenever auto-scroll is off
- Twitch IRC sometimes fails to init/connect on startup
- Ability to quickly open/navigate browser to a stream
- Reload emotes
- Handle "ACTION" (remove the word and display the message with italics)
- Temporarily pin a user's most recent 2-3 messages to top of window when clicking their name (also add option to do this automatically for users that get a lot of mentions over short duration)
- Cache json/images in a db file instead of loose files (also migrate away from using eframe persistence feature to save state)
- Poll status (Twitch, DGG)
- Handle Twitch CLEARCHAT, CLEARMSG commands
- Handle USERNOTICE command
- Twitch live status does not toggle off when stream goes offline

##

- Copying messages into clipboard and/or textbox (currently has minimal support -- can click on msg text to copy msg but no visual feedback. right click menu?)
- Mentions tab
- Twitch Prediction status
- Twitch tier-exclusive emote logic
- Zero width emote tiling option (e.g. scale to fit and paint X copies of the zero-width over the previous emote instead of stretching)

##

- Forced anti-spam option to supress duplicate message spam from users
- Used emote stats for ordering selector
- Allow message headers (stuff up to and including username) to split between rows properly
- DGG oauth issue - application oauth flow tokens not working but login keys created directly on DGG site work
- Multi-channel tabs?
- Loading animation image for unloaded emotes?
- Support/fix twitch modified emotes
- Better README
- Twitch Cheer emotes

##

- ~Select from a user list by starting typing a word with @~
- ~Click user (or usernames in chat messages) to see their recent messages in channel~
- ~DGG~
- ~Zero width emotes~
- ~Unicode character support~
- ~Twitch follower emotes~
- ~Detect ASCII art and new line appropiately regardless of width~