![](./demo.gif)

All the usual features you would expect in a Twitch chat app: channel tabs, emote support (Twitch/FFZ/BTTV/7TV/animated/zero-width), emote selector, etc...

Limited support for Youtube Live Stream chat but requires a Tampermonkey script (see below for instructions).

Also supports DGG chat. 

# Features

- Emote/User selector: 
  - Displays options automatically as you type. Start a word with @ to get user name selector instead of emote selector.
  - Use Tab and Shift-Tab to choose a emote/user
    - Can also use ALT + ←/→	to choose
- Can click a username to highlight their messages and get a popup overlay with their most recent few messages.
- Right click on message username to get option to copy the message to clipboard.
- Can split screen to display two chats at once via channel options (right click on channel tab) or dragging a channel tab to right half of messages area.

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

- Memory leaking
  - Seems to sometimes happen along with twitch chats not showing any new messages
  - Still happens with no emote or badge image loading
  - After enough time (was around 5Gb mem), focusing from minimized shows corrupted looking text and hard crash on attempting to change tab
    - Moving window to another monitor (dpi change?) fixes the corrupted text
  - High single thread CPU use when window is visible, even with only one chat tab (only after high memory use and/or corruption issue)
- Twitch emotes not refreshing properly (not picking up new emotes)
- Put exact match emote first in selector
- Websocket connections do not recover after being killed by VPN connecting
- Rarely closing app leaves config in invalid state, will crash on start until config file is deleted
- Twitch IRC sometimes fails to init/connect on startup
- Handle Twitch CLEARCHAT, CLEARMSG commands (and DGG MUTE/BAN)
- "Sub only" toggle option
- Option to download smaller/larger emote sizes
- Ability to open stream in browser for a selected tab
- Make it less likely for changes or new settings to break saved settings
- Clicking a username to see recent mentions make take multiple clicks to work when chat is moving fast
  - This might be tied to click sense requiring focus as well and focus resetting when chat window contents change
  - Labels do not support specifying an id for state persistence, maybe change clickable elements to button and style to match labels?

Might do:

- DGG Polls (Twitch sadly lacks API support that would allow chat apps to even see polls or predictions)
- Twitch tier-exclusive emote logic
- Twitch Cheer emotes
- DGG OAuth - tokens not working but login keys created directly on DGG site work
  - For now removed oauth flow and added text directing how to create a login key
- Collect stats for emotes used and use them to order emote selector?