function elementToMarkdown(root) {
  const children = node => [...node.childNodes].map(walk).join('');
  const fenceFor = text => '`'.repeat(Math.max(3, ...[...text.matchAll(/`+/g)].map(m => m[0].length + 1)));
  function walk(node) {
    if (node.nodeType === 3) return node.textContent;
    if (node.nodeType !== 1) return '';
    const tag = node.tagName.toLowerCase();
    if (node.classList.contains('code-block-decoration')) return '';
    if (['button', 'style', 'script', 'svg'].includes(tag) || node.getAttribute('aria-hidden') === 'true') return '';
    if (tag === 'pre') {
      const code = node.querySelector('code') || node;
      const value = code.textContent.replace(/\n$/, '');
      const language = [...code.classList].find(c => c.startsWith('language-'))?.slice(9) || '';
      const fence = fenceFor(value);
      return `\n\n${fence}${language}\n${value}\n${fence}\n\n`;
    }
    if (tag === 'code') {
      const fence = '`'.repeat(Math.max(1, ...[...node.textContent.matchAll(/`+/g)].map(m => m[0].length + 1)));
      return fence + node.textContent + fence;
    }
    if (tag === 'strong' || tag === 'b') return '**' + children(node) + '**';
    if (tag === 'em' || tag === 'i') return '*' + children(node) + '*';
    if (/^h[1-6]$/.test(tag)) return '\n\n' + '#'.repeat(Number(tag[1])) + ' ' + children(node).trim() + '\n\n';
    if (tag === 'br') return '\n';
    if (tag === 'hr') return '\n\n---\n\n';
    if (tag === 'a') {
      const href = node.getAttribute('href') || '';
      return /^(https?:|mailto:|#)/i.test(href) ? `[${children(node)}](${href.replace(/\)/g, '%29')})` : children(node);
    }
    if (tag === 'blockquote') return '\n\n' + children(node).trim().split('\n').map(line => '> ' + line).join('\n') + '\n\n';
    if (tag === 'ol' || tag === 'ul') {
      let next = Number(node.getAttribute('start') || 1);
      return '\n' + [...node.children].filter(n => n.tagName === 'LI').map(item => {
        if (item.hasAttribute('value')) next = Number(item.getAttribute('value'));
        const marker = tag === 'ol' ? `${next++}. ` : '- ';
        return marker + children(item).trim().split('\n').map((line, i) => i ? ' '.repeat(marker.length) + line : line).join('\n');
      }).join('\n') + '\n';
    }
    if (tag === 'table') {
      const rows = [...node.querySelectorAll('tr')].filter(row => row.closest('table') === node).map(row =>
        [...row.children].filter(cell => /^(TH|TD)$/.test(cell.tagName)).map(cell => children(cell).trim().replace(/\|/g, '\\|').replace(/\r?\n/g, '<br>')));
      if (!rows.length) return '';
      const width = Math.max(...rows.map(row => row.length));
      const line = row => '| ' + Array.from({ length: width }, (_, i) => row[i] ?? '').join(' | ') + ' |';
      return '\n\n' + [line(rows[0]), line(Array(width).fill('---')), ...rows.slice(1).map(line)].join('\n') + '\n\n';
    }
    const value = children(node);
    return tag === 'p' ? '\n\n' + value.trim() + '\n\n' : value;
  }
  return walk(root).trim();
}
