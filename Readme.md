# Features

All the usual basic features you would expect in a twitch chat app: channel tabs, all the emotes (twitch/ffz/bttv/7tv/animated/zero-width/etc), emote selector, etc...

# Todo

- Ability to quickly open/navigate browser to a stream
- Reload emotes
- Click user (or usernames in chat messages) to see their recent messages in channel
- Cache json/images in a db file instead of loose files
- Poll status (Twitch, DGG)
- Handle Twitch CLEARCHAT, CLEARMSG commands
- Handle USERNOTICE command
- Twitch live status does not toggle off when stream goes offline

##

- Copying messages into clipboard and/or textbox (partial support -- can click on msg text to copy msg but no visual feedback)
- Mentions tab
- Twitch Prediction status
- Twitch tier-exclusive emote logic
- Zero width emote tiling option (e.g. scale to fit and paint X copies of the zero-width over the previous emote instead of stretching)

##

- Used emote stats for ordering selector
- Allow message headers (stuff up to and including username) to split between rows properly
- DGG oauth issue - application oauth flow tokens not working but login keys created directly on DGG site work
- Multi-channel tabs?
- Loading animation image for unloaded emotes?
- Support/fix twitch modified emotes
- Combo tooltip to show list of users
- Better README
- Twitch Cheer emotes

##

- ~Zero width emotes~
- ~Unicode character support~
- ~Twitch follower emotes~
- ~Detect ASCII art and new line appropiately regardless of width~