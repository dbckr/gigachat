All the usual features you would expect in a twitch chat app: channel tabs, all the emotes (twitch/ffz/bttv/7tv/animated/zero-width/etc), emote selector, etc...

Also DGG support.

Will add Youtube support if they ever make their live stream chat API more accessible...

# Features

- Emote/User selector: 
  - Displays options automatically as you type. Start a word with @ to get user name selector instead of emote selector.
  - ALT ←/→	to choose a emote/user and ALT ↓ to select (working on an option to use TAB & ENTER like DGG chat, having some problems with the UI framework)
- Right click a chat message to get option to copy it to clipboard, or left click on it to directly copy it. Must click on a section of plain text, not an emote or link.
- Can click a username to highlight their messages and get a popup with their most recent few messages.

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