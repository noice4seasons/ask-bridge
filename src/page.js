function pageState({ action = 'status', model } = {}) {
  if (location.hostname !== 'gemini.google.com') throw new Error('页面已离开 Gemini，请自行完成登录。');
  const visible = el => !!el && el.getBoundingClientRect().width > 0 && el.getBoundingClientRect().height > 0 && getComputedStyle(el).visibility !== 'hidden';
  const editor = [...document.querySelectorAll('[role="textbox"][contenteditable="true"],rich-textarea [contenteditable="true"],.ql-editor[contenteditable="true"]')].find(visible);
  const picker = document.querySelector('[data-test-id="bard-mode-menu-button"]');
  const items = [...document.querySelectorAll('[data-test-id="gem-mode-menu"] [role="menuitem"], [data-test-id^="bard-mode-option-"]')].filter(visible);
  const label = el => el.innerText.trim().split('\n')[0].trim();
  const options = [...new Set(items.map(label))].filter(t => /\d|Pro|Flash|Argon/i.test(t));
  const reasoningItems = [...document.querySelectorAll('[role="menuitem"]')].filter(visible).filter(e=>/^(Low|Medium|High)$/.test(label(e)));
  const reasoningOptions = reasoningItems.map(label);
  const selectedReasoning = reasoningItems.filter(e=>e.classList.contains('selected')).map(label)[0] || null;
  const authRequired = [...document.querySelectorAll('a[href*="accounts.google.com"]')].some(e => visible(e) && /^(Sign in|登录|登录)$/i.test(e.innerText.trim()));
  const responses = document.querySelectorAll('model-response');
  if (action === 'status') return { ready: !!editor && !!picker && !authRequired, authRequired, model: picker?.innerText.trim() || null, options, reasoningOptions, selectedReasoning, responses: responses.length, url: location.origin + location.pathname };
  let target;
  if (action === 'picker') target = picker;
  if (action === 'model' || action === 'reasoning') {
    const norm = s => s.toLowerCase().replace(/[\s.\-_]/g, '');
    target = items.find(e => norm(label(e)) === norm(model));
    if (action === 'reasoning') target = [...document.querySelectorAll('[role="menuitem"]')].filter(visible).find(e => label(e).trim() === model || e.innerText.trim().split(/\s+/)[0] === model);
    if (!target) throw new Error('指定模型不可用：' + model + '；可见选项：' + options.join(', '));
  }
  if (action === 'focus') {
    if (!editor || editor.innerText.trim()) throw new Error('编辑器未就绪或已有草稿，未覆盖。');
    editor.focus(); return { focused: document.activeElement === editor };
  }
  if (action === 'draft') return { text: editor?.innerText || '', responses: responses.length };
  if (action === 'send') target = [...document.querySelectorAll('button')].find(e => visible(e) && /^(Send message|Send|发送消息|发送|傳送訊息|傳送|提交)$/i.test(e.getAttribute('aria-label') || '')) || document.querySelector('button[data-test-id="send-button"]');
  if (!visible(target) || target.disabled || target.getAttribute('aria-disabled') === 'true') throw new Error('操作控件未就绪：' + action);
  const rect = target.getBoundingClientRect();
  return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
}
