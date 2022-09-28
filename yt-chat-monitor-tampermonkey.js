// based heavily on
// https://greasyfork.org/en/scripts/409664-chat-filter-for-youtube-live/code

// ==UserScript==
// @name         Gigachat Youtube Chat Integration
// @namespace    http://tampermonkey.net/
// @version      0.1
// @description  try to take over the world!
// @author       dbckr
// @match        https://www.youtube.com/watch?v=*
// @icon         data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==
// @grant        GM.xmlHttpRequest
// @run-at       document-start
// ==/UserScript==
(function(){
  const LIVE_PAGE = {
      getChatField: ()=>{
          let chatField
          if(document.getElementById('chatframe')!==null){
              chatField = document.getElementById('chatframe').contentDocument.querySelector("#items.style-scope.yt-live-chat-item-list-renderer")
          }else{
              chatField = document.querySelector("#items.style-scope.yt-live-chat-item-list-renderer")
          }
          return chatField
      }
  }

 window.onload = function() {
      setTimeout(function(){
          findChatField()
      }, 1000)
  };

  let storedHref = location.href;
  const URLObserver = new MutationObserver(function(mutations){
      mutations.forEach(function(mutation){
          if(storedHref !== location.href){
              findChatField()
              storedHref = location.href
              log('URL Changed', storedHref, location.href)
          }
      })
  })
  
  URLObserver.disconnect()
  URLObserver.observe(document, {childList: true, subtree: true})

  var findInterval
  const findChatField = () =>{
      let FindCount = 1
      clearInterval(findInterval)
      findInterval = setInterval(function(){
          FindCount++
          if(FindCount > 180){
              log('The element cannot be found')
              clearInterval(findInterval)
              FindCount = 0
          }
          if(document.getElementById('chatframe')){
              if(LIVE_PAGE.getChatField() !== null){
                  log('Found the element: ')
                  console.log(LIVE_PAGE.getChatField())

                  initialize()

                  clearInterval(findInterval)
                  FindCount = 0
             }
          }
      }, 1000)
  }

  const initialize = () =>{
      log('initialize...')
      if(LIVE_PAGE.getChatField() !== null){
          ChatFieldObserver.disconnect()
          ChatFieldObserver.observe(LIVE_PAGE.getChatField(), {childList: true})
      }
  }

  const convertChat = (chat) => {
    let message = ''
    let authorID
    let userName

    let children = Array.from(chat.children)
    children.some(_chat =>{

        let childID = _chat.id

        if(childID === 'content'){
            let _message = Array.from(_chat.children).find((v) => v.id === 'message')
            /*let textChildren = _message.innerHTML.split(/<img|">/g)
            textChildren.some(_text => {
                if(_text.match('emoji style-scope yt-live-chat-text-message-renderer')){
                    message += _text.alt
                }else{//テキストの場合
                    message += _text
                }
            })*/
            message = _message.innerHTML;
            let messageImgs = Array.from(_message.children);
            messageImgs.some(_img => {
              message = message.replace(_img.outerHTML, _img.alt);
            });
            userName = _chat.children[1].innerText;
        }

        /*if(childID === 'author-photo'){
            let str = _chat.lastElementChild.getAttribute('src')||''
            let result = str.split('/')

            //yt3.ggpht.com/【-xxxxxxxxxxx】/AAAAAAAAAAI/AAAAAAAAAAA/【xxxxxxxxxxx】/s32-c-k-no-mo-rj-c0xffffff/photo.jpg
            authorID = result[3]+result[6]
        }*/
    })

    let result={
        message: message,
        authorID: authorID,
        userName: userName
    }
    return result
}

  const ChatFieldObserver = new MutationObserver(function(mutations){
      mutations.forEach(function(e){
          let addedChats = e.addedNodes
          if(addedChats.length <= 0){
              return
          }

          for(let i = 0; i < addedChats.length; i++){
              //.yt-live-chat-placeholder-item-rendererを避ける
              if(addedChats[i].children.length <= 0){
                  continue
              }

              const chatData = convertChat(addedChats[i])
              var request = JSON.stringify({
                username: chatData.userName,
                message: chatData.message
              });
              GM.xmlHttpRequest({
                method: "POST",
                headers: {
                    "User-Agent": "derp",
                    "Content-Type": "application/json"
                },
                url: "http://localhost:8008/incoming-msg",
                data: request,
                dataType: "json",
                contentType: 'application/json',
                onload: function (response) {
                }
              });
          }
      })
  })

  //------------------------------------------
  const log = (mes) => {console.log('【CFY】'+mes)}
})()