function waitForAnswer({ before, begin, end, timeout }) {
  if (location.hostname !== 'gemini.google.com') throw new Error('不是 Gemini 页面。');
  return new Promise((resolve, reject) => {
    let lastText = ''; let stableSince = Date.now(); let throttle; let evaluations = 0;
    const scope = document.querySelector('chat-window, main') || document.body;
    const visible = el => el.getBoundingClientRect().width > 0 && el.getBoundingClientRect().height > 0;
    function cleanup() { observer.disconnect(); clearTimeout(deadline); clearTimeout(throttle); clearInterval(fallback); }
    function check() {
      evaluations++;
      if (location.hostname !== 'gemini.google.com') { cleanup(); reject(new Error('页面已离开 Gemini。')); return; }
      const messages = [...document.querySelectorAll('model-response')];
      if (messages.length <= before) return;
      const latest = messages.at(-1);
      const content = latest.querySelector('.markdown, message-content') || latest;
      const text = content.textContent || '';
      if (text !== lastText) { lastText = text; stableSince = Date.now(); }
      const stop = [...document.querySelectorAll('button')].some(e => visible(e) && /stop response|stop generating|停止生成|停止回應|停止回答/i.test(e.getAttribute('aria-label') || ''));
      const copy = [...latest.querySelectorAll('button')].some(e => /copy|複製|复制/i.test(e.getAttribute('aria-label') || '') && !/code|代码|程式碼/i.test(e.getAttribute('aria-label') || ''));
      if (text.includes(begin) && text.includes(end) && !stop && copy && Date.now() - stableSince >= 1500) {
        cleanup(); resolve({ count: messages.length, evaluations, url: location.origin + location.pathname });
      }
      const alert = [...scope.querySelectorAll('[role="alert"]')].map(e => e.innerText).join(' ');
      if (/limit|quota|额度|配額|上限|try again later/i.test(alert)) { cleanup(); reject(new Error('Gemini 显示额度或服务限制，未切换模型。')); }
    }
    const observer = new MutationObserver(() => { if (!throttle) throttle = setTimeout(() => { throttle = null; check(); }, 250); });
    observer.observe(scope, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['aria-label', 'disabled', 'aria-disabled'] });
    const fallback = setInterval(check, 1500);
    const deadline = setTimeout(() => { cleanup(); reject(new Error('回答尚未完整完成；请稍后使用 get 取回同一工作，勿重新提交。')); }, timeout);
    check();
  });
}
