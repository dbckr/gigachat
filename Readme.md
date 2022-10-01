![](./demo.gif)

All the usual features you would expect in a Twitch chat app: channel tabs, emote support (Twitch/FFZ/BTTV/7TV/animated/zero-width/etc), emote selector, etc...

Limited support for Youtube Live Stream chat but requires a Tampermonkey script (see below for instructions).

Also supports DGG chat. 

# Features

- Emote/User selector: 
  - Displays options automatically as you type. Start a word with @ to get user name selector instead of emote selector.
  - ALT ←/→	to choose a emote/user and ALT ↓ to select (working on an option to use TAB & ENTER like DGG chat, having some problems with the UI framework)
- Right click a chat message to get option to copy it to clipboard, or left click on it to directly copy it. Must click on a section of plain text, not an emote or link.
- Can click a username to highlight their messages and get a popup with their most recent few messages.
- Can split screen to display two chats at once via channel options (right click channel tab) or dragging a channel tab to right half of messages area.

# YouTube Live Chat Integration

Hacky but functional support for YT chatting within the app by using a Tampermonkey script and embedded web server:

- Install Tampermonkey extension in your browser of choice.
- In Tampermonkey, create a new script, paste in the contents of the yt-chat-monitor-tampermonkey.js file in the root of this repo, and save it.
- Turn on "Enable YT Integration" in Gigachat options menu.
  - This starts an embedded web server that listens on port 36969. The Tampermonkey script will use it to send and receive chat messages.
- When you open a YT live stream it should detect the live chat UI and start sending messages over to the app, and also send messages typed into the app.
  - Currently the script is just scraping the HTML of the live chat panel, so hiding chat or popping it out will probably break it. But switching to theater mode to move the chat below the video works fine.
- Gigachat will create a chat tab for each open YT stream automatically. These tabs will remain until you remove them.

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
- DGG OAuth - tokens not working but login keys created directly on DGG site work
  - Removed oauth flow and added text directing how to create a login key
- Collect stats for emotes used and use them to order emote selector?