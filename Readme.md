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

Bugs:

- Clicking a username to see recent mentions or right clicking to open context menu can take multiple clicks to work when chat is moving fast
  - This might be tied to click sense requiring focus as well and focus resetting when chat window contents change
  - Labels do not support specifying an id for state persistence, maybe change clickable elements to button and style to match labels?
- Still have some positioning calc issues i.e. flickering from scroll bar jumping around, but only when message timestamps are turned off
- Memory leak behavior (rare, into low GBs after many days)
  - Tested to occur eventually even with all image loading disabled
  - After enough time (was around 5Gb mem), focusing from minimized shows corrupted looking text and hard crash on attempting to change tab
    - Moving window to another monitor (dpi change?) fixes the corrupted text
  - High single thread CPU use when window is visible, even with only one chat tab (only after high memory use and/or corruption issue)
- Twitch IRC sometimes fails to init/connect on startup
- Websocket connections do not recover after being killed by VPN connecting

Planned Features:

Low priority / Might do:

- Support BTTV Emote Modifiers via an option toggle (e.g. w! v! h! z!)
- Button to open stream in browser for a selected tab/channel
- Option to download smaller/larger emote sizes
- DGG Polls (Twitch sadly lacks API support for polls or predictions)
- Twitch tier-exclusive emote logic
- DGG OAuth - tokens not working but login keys created directly on DGG site work
  - For now removed oauth flow and open brower to DGG site instead