function(args) {
 if(location.hostname!=='gemini.google.com')throw new Error('不是 Gemini 页面');
 const messages=[...document.querySelectorAll('model-response')];
 if(messages.length<=args.before)throw new Error('尚无本次新回答');
 const content=messages.at(-1).querySelector('.markdown, message-content')||messages.at(-1);
 const raw=(__MARKDOWN_FUNCTION__)(content);
 return {raw,url:location.origin+location.pathname,count:messages.length};
}
