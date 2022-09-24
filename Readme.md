![](./demo.gif)

All the usual features you would expect in a twitch chat app: channel tabs, emote support (twitch/ffz/bttv/7tv/animated/zero-width/etc), emote selector, etc...

Also supports DGG chat. Will add support for Youtube live stream chat if YT ever makes that API more feasible to support...

# Features

- Emote/User selector: 
  - Displays options automatically as you type. Start a word with @ to get user name selector instead of emote selector.
  - ALT ←/→	to choose a emote/user and ALT ↓ to select (working on an option to use TAB & ENTER like DGG chat, having some problems with the UI framework)
- Right click a chat message to get option to copy it to clipboard, or left click on it to directly copy it. Must click on a section of plain text, not an emote or link.
- Can click a username to highlight their messages and get a popup with their most recent few messages.
- Can split screen to display two chats at once via channel options or dragging a tab to right half of messages area.

# Todo

- Twitch IRC sometimes fails to init/connect on startup
- Handle Twitch CLEARCHAT, CLEARMSG commands (and DGG MUTE/BAN)
- "Sub only" toggle option
- Option to download smaller emote sizes
- Ability to open stream in browser for a selected tab
- Make it less likely for changes or new settings to break saved settings

Might do:

- DGG Polls (Twitch sadly lacks API support that would allow chat apps to even see polls or predictions)
- Twitch tier-exclusive emote logic
- Support/fix twitch modified emotes
- Twitch Cheer emotes
- Zero width emote tiling option (e.g. scale to fit and paint X copies of the zero-width over the previous emote instead of stretching)
- DGG OAuth - tokens not working but login keys created directly on DGG site work
  - Removed oauth flow and added text directing how to create a login key
- Collect stats for emotes used and use them to order emote selector?