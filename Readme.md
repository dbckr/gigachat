# Features

All the usual basic features you would expect in a twitch chat app: channel tabs, all the emotes (twitch/ffz/bttv/7tv/animated/zero-width/etc), emote selector, etc...

# Tips

- Emote/User selector: 
  - Displays options automatically as you type. Type @ and a few characters in a username to get user name selector instead of emote selector.
  - ALT ←/→	to choose a emote/user and ALT ↓ to select (working on an option to use TAB & ENTER like DGG chat, having some problems with the UI framework)
- Can right click a message to get option to copy it to clipboard, or left click on it to directly copy it. Must click on a section of plain text, not an emote or link.

# Todo

- Better README
- Twitch IRC sometimes fails to init/connect on startup
- Option to download smaller emote size
- Ability to quickly open/navigate browser to a stream
- Handle Twitch CLEARCHAT, CLEARMSG commands
- Handle USERNOTICE command
- Make non-breaking changes to persistable configuration not break old configuration (or add automatic configuration migration)
- Poll status (Twitch, DGG)
- Twitch Prediction status
- Twitch tier-exclusive emote logic
- Used emote stats for ordering selector
- Allow message headers (stuff up to and including username) to split between rows properly
- Support/fix twitch modified emotes
- Twitch Cheer emotes
- Zero width emote tiling option (e.g. scale to fit and paint X copies of the zero-width over the previous emote instead of stretching)
- DGG oauth issue - application oauth flow tokens not working but login keys created directly on DGG site work
  - Current workaround: removed oauth flow and add text directing users to create and enter a login key

##

- ~Handle "ACTION" (remove the word and display the message with italics)~
- ~Mentions tab~
- ~Reload emotes~
- ~Copying messages into clipboard and/or textbox (currently has minimal support -- can click on msg text to copy msg but no visual feedback. right click menu?)~
- ~Display a "button" to jump back to bottom whenever auto-scroll is off~
- ~Temporarily pin a user's most recent 2-3 messages to top of window when clicking their name~ 
  - Add option to do this automatically for users that get a lot of mentions over short duration
- ~Select from a user list by starting typing a word with @~
- ~Click user (or usernames in chat messages) to see their recent messages in channel~
- ~DGG~
- ~Zero width emotes~
- ~Unicode character support~
- ~Twitch follower emotes~
- ~Detect ASCII art and new line appropiately regardless of width~
- ~Emote textures can go missing after some time (did a recent egui update add texture expiration??)- Emote textures can go missing after some time (did a recent egui update add texture expiration??)~