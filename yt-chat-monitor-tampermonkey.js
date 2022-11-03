// based heavily on
// https://greasyfork.org/en/scripts/409664-chat-filter-for-youtube-live/code

// ==UserScript==
// @name         Gigachat Youtube Chat Integration
// @namespace    http://tampermonkey.net/
// @version      0.1
// @description  try to take over the world!
// @author       dbckr
// @match        https://www.youtube.com/watch*
// @icon         data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==
// @grant        GM.xmlHttpRequest
// @run-at       document-start
// ==/UserScript==

// shouldn't need this
// @require      https://code.jquery.com/jquery-3.6.1.min.js

var channelName = null;
(function() {
  const LIVE_PAGE = {
      getChatField: ()=>{
          let chatField
          if(document.getElementById('chatframe')!==null){
              chatField = document.getElementById('chatframe').contentDocument.querySelector("#items.style-scope.yt-live-chat-item-list-renderer")
          }else{
              chatField = document.querySelector("#items.style-scope.yt-live-chat-item-list-renderer")
          }
          return chatField
      },
      getChatInput: ()=>{
          let chatInput
          if(document.getElementById('chatframe')!==null){
              chatInput = document.getElementById('chatframe').contentDocument.querySelector("div#input.style-scope.yt-live-chat-text-input-field-renderer")
          }else{
              chatInput = document.querySelector("div#input.style-scope.yt-live-chat-text-input-field-renderer")
          }
          return chatInput
      },
      getChatButton: ()=>{
          let chatButton
          if(document.getElementById('chatframe')!==null){
              chatButton = document.getElementById('chatframe').contentDocument.querySelector('button#button[aria-label="Send"]')
          }else{
              chatButton = document.querySelector('button#button[aria-label="Send"]')
          }
          return chatButton
      },
      getChannelName: ()=>{
          return document.querySelector("ytd-channel-name a").innerText
      }
  }

 window.onload = function() {
  setTimeout(function() {
      findChatField()
  }, 1000)
 };

  let storedHref = location.href;
  const URLObserver = new MutationObserver(function(mutations) {
      mutations.forEach(function(mutation) {
          if (storedHref !== location.href) {
              findChatField()
              storedHref = location.href
              log('URL Changed', storedHref, location.href)
          }
      })
  })

  URLObserver.disconnect()
  URLObserver.observe(document, {childList: true, subtree: true})

  var findInterval
  const findChatField = () => {
    let FindCount = 1
    clearInterval(findInterval)
    findInterval = setInterval(function() {
      FindCount++
      if(FindCount > 180) {
        log('The element cannot be found')
        clearInterval(findInterval)
        FindCount = 0
      }
      //if(document.getElementById('chatframe')){
        if(LIVE_PAGE.getChatField() !== null/* && LIVE_PAGE.getChatInput() !== null*/) {
            log('Found the element: ')
            log(LIVE_PAGE.getChatField())
            initialize()
            clearInterval(findInterval)
            FindCount = 0
        }
      //}
    }, 1000)
  }

  var checkInterval
  const checkChatField = () => {
    clearInterval(checkInterval)
    checkInterval = setInterval(function() {
      if (LIVE_PAGE.getChatField() === null) {
        clearInterval(checkInterval)  
        findChatField()
      }
    }, 1000)
  }

  const initialize = () =>{
      log('initialize...')
      channelName = LIVE_PAGE.getChannelName()
      if (LIVE_PAGE.getChatField() !== null) {
        ChatFieldObserver.disconnect()
        ChatFieldObserver.observe(LIVE_PAGE.getChatField(), {childList: true})
      }

      checkChatField()

      function queryForOutput() {
        if (!waiting) {
          try {
            var url = "http://localhost:36969/outgoing-msg/" + encodeURIComponent(channelName);
            //log('in');
            waiting = true;
            GM.xmlHttpRequest({
              method: "GET",
              headers: {
                  "User-Agent": "derp",
                  "Content-Type": "application/json"
              },
              url: url,
              timeout: 11000,
              onerror: function (err) {
                //log('queryForOutput error');
                waiting = false;
                setTimeout(queryForOutput, 3000);
              },
              onload: function (response) {
                try {
                  var msg = JSON.parse(response.responseText);
                  if (msg && msg.message && msg.message.length > 0) {
                    if (LIVE_PAGE.getChatInput() !== null && LIVE_PAGE.getChatButton() !== null) {
                      LIVE_PAGE.getChatInput().textContent = msg.message;
                      LIVE_PAGE.getChatInput().dispatchEvent(new InputEvent('input', {
                          bubbles: true,
                          data: msg.message,
                          inputType: "insertText",
                          returnValue: true,
                          type: "input",
                          which: 0
                      }))
                      LIVE_PAGE.getChatButton().click();
                    }
                    else {
                      var request = JSON.stringify({
                        username: "",
                        message: "You do not have permission to post in this chat",
                        role: "error",
                        channel: channelName
                      });
                      GM.xmlHttpRequest({
                        method: "POST",
                        headers: {
                            "User-Agent": "derp",
                            "Content-Type": "application/json"
                        },
                        url: "http://localhost:36969/incoming-msg",
                        data: request,
                        dataType: "json",
                        contentType: 'application/json',
                        onload: function (response) {
                        }
                      });
                    }
                  } 
                }
                catch (err){
                  log('error processing response: ' + err);
                }
                finally { 
                  //log('out');
                  waiting = false;
                  setTimeout(queryForOutput, 50);
                }
              }
            });
          }
          catch { log('err'); waiting = false; setTimeout(queryForOutput, 1000); }
        }
      }

      if (!waiting) {
        setTimeout(queryForOutput, 500);
      }
    }

  const convertChat = (chat) => {
    let message = ''
    let authorID
    let userName
    let role
    let badges = []
    let emotes = []

    let children = Array.from(chat.children)
    children.some(_chat => {

      let childID = _chat.id

      if (childID === 'content') {
          let _message = Array.from(_chat.children).find((v) => v.id === 'message')
          message = _message.innerHTML;
          let messageElements = Array.from(_message.children);
          messageElements.some(_ele => {
            if (_ele.nodeName === "IMG") {
              // replace emote <img> with emote text
              message = message.replace(_ele.outerHTML, ' ' + _ele.alt + ' ');
              emotes.push({ name: _ele.alt, src: _ele.src });
            }
            else {
              // strip link tag
              message = message.replace(_ele.outerHTML, _ele.href);
            }
          });
          //message = message.replace(/\s+/, ' ');
          userName = _chat.children[1].innerText;
          if (_chat.children[1].children[0].classList.contains('moderator') 
            || _chat.children[1].children[1] && _chat.children[1].children[1].classList.contains('moderator')){
              role = "moderator";
          }
          else if (_chat.children[1].children[0].classList.contains('member') 
            || _chat.children[1].children[1] && _chat.children[1].children[1].classList.contains('member')){
              role = "member";
          }
      }
    })

    let result = {
      message: message,
      authorID: authorID,
      userName: userName,
      role: role,
      emotes: emotes
    }
    return result
}

  const ChatFieldObserver = new MutationObserver(function(mutations) {
      mutations.forEach(function(e) {
          let addedChats = e.addedNodes
          if (addedChats.length <= 0){
              return
          }

          for (let i = 0; i < addedChats.length; i++) {
              if (addedChats[i].children.length <= 0) {
                  continue
              }

              const chatData = convertChat(addedChats[i])
              var request = JSON.stringify({
                username: chatData.userName,
                message: chatData.message,
                role: chatData.role,
                emotes: chatData.emotes,
                channel: channelName
              });
              //log('send msg: ' + chatData.userName);
              //log('request: ' + request);
              GM.xmlHttpRequest({
                method: "POST",
                headers: {
                    "User-Agent": "derp",
                    "Content-Type": "application/json"
                },
                url: "http://localhost:36969/incoming-msg",
                data: request,
                dataType: "json",
                contentType: 'application/json',
                timeout: 1000,
                onerror: function (err) {

                },
                onload: function (response) {
                  //log('response: ' + JSON.stringify(response));
                }
              });
          }
      })
  })

  var waiting = false;

  //------------------------------------------
  const log = (mes) => {console.log('【yt-chat-monitor】'+mes)}
})()