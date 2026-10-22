import { describe, it, expect } from 'vitest';
import { cleanEditableHtml, extractEditableText } from '../richEditor';

/** 用 div 当 iframe body 的替身:这两个函数只用到 cloneNode / innerHTML /
 *  textContent 等 body 与 div 共有的能力(jsdom 没有 innerText)。 */
function bodyOf(html: string): HTMLElement {
  const host = document.createElement('div');
  host.innerHTML = html;
  return host;
}

describe('cleanEditableHtml', () => {
  it('drops the source page layout and color noise', () => {
    // 深色聊天界面的青蓝底白字 + 工具输出的 grid 两列 / 内层滚动 / sticky 标签,
    // 这些是"别人页面的上下文",存进 CF_HTML 就是错位与怪配色
    const out = cleanEditableHtml(
      bodyOf(
        '<div style="display:grid;grid-template-columns:1fr 1fr;max-height:150px;' +
          'overflow-y:auto;position:sticky;font-size:11px;background-color:rgb(81,173,204);' +
          'color:rgb(249,250,251);margin:32px 0;padding:8px;width:600px;' +
          'transform:translateY(2px);opacity:0.9">正文</div>',
      ),
    );
    expect(out).toContain('正文');
    for (const banned of [
      'background-color',
      'display:grid',
      'max-height',
      'overflow',
      'position:sticky',
      'font-size',
      'color:',
      'margin',
      'padding',
      'width:',
      'transform',
      'opacity',
    ]) {
      expect(out).not.toContain(banned);
    }
  });

  it('keeps semantic tags and semantic inline formatting', () => {
    const out = cleanEditableHtml(
      bodyOf(
        '<p><strong>粗</strong><em>斜</em><u>下划</u><s>删</s><code>inline</code>' +
          '<span style="font-weight:700">粗体span</span>' +
          '<span style="font-style:italic">斜体span</span>' +
          '<span style="text-decoration:underline">下划线span</span></p>' +
          '<pre>代码块\n第二行</pre><ul><li>项</li></ul><ol><li>一</li></ol>' +
          '<blockquote>引用</blockquote><h2>标题</h2>' +
          '<a href="https://example.com/x" class="link" target="_blank">链</a>',
      ),
    );
    for (const tag of [
      '<strong>',
      '<em>',
      '<u>',
      '<s>',
      '<code>',
      '<pre>',
      '<ul>',
      '<ol>',
      '<li>',
      '<blockquote>',
      '<h2>',
    ]) {
      expect(out).toContain(tag);
    }
    // 只有语义格式留下,且粗细归一到 600
    expect(out).toContain('font-weight:600');
    expect(out).toContain('font-style:italic');
    expect(out).toContain('text-decoration:underline');
    // 属性白名单:href 留,class/target 删
    expect(out).toContain('<a href="https://example.com/x">');
    expect(out).not.toContain('class=');
    expect(out).not.toContain('target=');
  });

  it('turns a white-space:pre-wrap span into a real pre block', () => {
    // 来源用 <span style="white-space: pre-wrap"> 模拟代码块 —— 没有语义,
    // 换成 <pre>,内部换行与缩进原样保留
    const out = cleanEditableHtml(
      bodyOf('<div><span style="white-space: pre-wrap">第一行\n第二行\n  缩进</span></div>'),
    );
    expect(out).toBe('<div><pre>第一行\n第二行\n  缩进</pre></div>');
  });

  it('keeps only inline data images', () => {
    const out = cleanEditableHtml(
      bodyOf(
        '<p><img src="https://evil.example/x.png" alt="外链">' +
          '<img src="data:image/png;base64,AAAA" alt="内嵌"></p>',
      ),
    );
    expect(out).not.toContain('evil.example');
    expect(out).toContain('src="data:image/png;base64,AAAA"');
    expect(out).toContain('alt="内嵌"');
  });

  it('drops comments, document-level tags and editor bookkeeping attributes', () => {
    const out = cleanEditableHtml(
      bodyOf(
        '<style>p{color:red}</style><!-- 注释 -->' +
          '<p onclick="x()" contenteditable="true" spellcheck="false" draggable="true" ' +
          'data-foo="1" id="a" class="b">正文</p><script>alert(1)</script>',
      ),
    );
    expect(out).toContain('<p>正文</p>');
    for (const banned of [
      '注释',
      '<style',
      '<script',
      'onclick',
      'contenteditable',
      'spellcheck',
      'draggable',
      'data-foo',
      'id=',
      'class=',
    ]) {
      expect(out).not.toContain(banned);
    }
  });

  it('removes empty spans the editor leaves behind', () => {
    const out = cleanEditableHtml(
      bodyOf(
        '<p>前<span></span>后<span><img src="data:image/png;base64,AA"></span><span> </span></p>',
      ),
    );
    expect(out).toContain('前后');
    expect(out).toContain('<span><img src="data:image/png;base64,AA"></span>');
    expect(out).not.toContain('<span></span>');
    expect(out).not.toContain('<span> </span>');
  });
});

describe('extractEditableText', () => {
  it('normalizes NBSP, CRLF and trailing newlines', () => {
    const body = bodyOf('');
    // 直接写 textContent:走 innerHTML 解析的话 HTML 预处理会先把 CRLF 变成 LF,
    // 就测不到换行归一这条了
    body.textContent = '第一行\u00a0尾\r\n第二行\r\n\r\n';
    expect(extractEditableText(body)).toBe('第一行 尾\n第二行');
  });

  it('keeps inner newlines and indentation', () => {
    const body = bodyOf('');
    body.textContent = 'if (x) {\n  return 1;\n}';
    expect(extractEditableText(body)).toBe('if (x) {\n  return 1;\n}');
  });
});
