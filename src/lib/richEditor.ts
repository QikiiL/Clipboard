/** 富文本编辑态的取回:HTML 归一 + 纯文本提取(纯函数,便于单测)。
 *
 *  弹窗(ContentEditorDialog)把 iframe 切到 designMode 后,用户敲出来的 DOM
 *  混着两类噪音:
 *  - 浏览器编辑行为留下的属性:`contenteditable` / `spellcheck` / `draggable`、
 *    编辑命令加的 `class`/`id`、浏览器扩展或粘贴来源塞的 `data-*`;
 *  - 粘贴来源页面的内联样式:深色聊天界面的 `background-color`/`color`、
 *    工具输出的 `display:grid` / `max-height` / `overflow` / `position:sticky`、
 *    以及各种 `font-size` / `font-family` / 宽高 / transform —— 这些是"别人页面
 *    的布局上下文",不是内容。
 *
 *  所以这里只留**语义格式**(粗细/斜体/下划线删除线/对齐/上下标),其余全丢。
 *  后端还会再净化一次(以及写 CF_HTML),这里是第一道闸。
 *
 *  为什么以 iframe body 为入参而不是字符串:调用方手上正好是
 *  `iframe.contentDocument.body`(真 DOM),克隆后处理比正则改字符串可靠得多。
 */

/** 元素上唯一允许保留的属性(其余一律删除) */
const KEEP_ATTRS = new Set(['href', 'src', 'alt', 'title', 'colspan', 'rowspan']);

/** 文档级/样式级标签:片段里出现即删(它们不是内容) */
const DROP_ELEMENTS = 'style,script,link,meta,title,base';

/** 从 iframe body 取回可保存的 HTML 片段。
 *  返回 `body` 克隆的 innerHTML —— 只含语义标签与语义格式。 */
export function cleanEditableHtml(body: HTMLElement): string {
  const clone = body.cloneNode(true) as HTMLElement;

  removeCommentNodes(clone);
  clone.querySelectorAll(DROP_ELEMENTS).forEach((el) => el.remove());

  // 必须在清属性之前做:判定依据正是来源内联的 white-space
  convertPreWrapBlocks(clone);

  clone.querySelectorAll('*').forEach((el) => sanitizeElement(el));

  removeEmptySpans(clone);

  return clone.innerHTML;
}

/** 从 iframe body 取回纯文本:保留换行与缩进,统一空白字符。
 *  `innerText` 是"渲染后"的文本(块级元素自带换行),Chrome/WebView2 有;
 *  jsdom 没有实现,回落 `textContent`(单测环境)。 */
export function extractEditableText(body: HTMLElement): string {
  const raw = typeof body.innerText === 'string' ? body.innerText : (body.textContent ?? '');
  return raw
    // 编辑态里的 &nbsp; 是排版产物,存纯文本时还原成普通空格
    .replace(/\u00a0/g, ' ')
    // CRLF / 孤立 CR → LF
    .replace(/\r\n?/g, '\n')
    // 末尾的空白与换行(回车留下的空段)不入库
    .replace(/\s+$/, '');
}

function removeCommentNodes(root: Node): void {
  const doc = root.ownerDocument;
  if (!doc) return;
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_COMMENT);
  const comments: Node[] = [];
  while (walker.nextNode()) comments.push(walker.currentNode);
  comments.forEach((node) => node.parentNode?.removeChild(node));
}

/** 来源里用 `white-space: pre-wrap`(或 pre)的 span/div 模拟代码块 ——
 *  它们没有语义,换成真正的 `<pre>`;内容(含换行)原样搬过去。
 *  `code` / 已有 `pre` 内部的元素不动(那已经是语义了)。 */
function convertPreWrapBlocks(root: HTMLElement): void {
  const candidates = Array.from(root.querySelectorAll('span,div')).filter((el) => {
    if (el.closest('pre,code')) return false;
    const ws = ((el as HTMLElement).style?.whiteSpace ?? '').trim().toLowerCase();
    return ws === 'pre-wrap' || ws === 'pre';
  });
  // 只转最外层:内层候选随外层一起成为 <pre> 的内容,避免 pre 套 pre
  const outermost = candidates.filter(
    (el) => !candidates.some((other) => other !== el && other.contains(el)),
  );
  for (const el of outermost) {
    const pre = el.ownerDocument.createElement('pre');
    while (el.firstChild) pre.appendChild(el.firstChild);
    el.replaceWith(pre);
  }
}

function sanitizeElement(el: Element): void {
  const style = (el as HTMLElement).style;
  const keptStyle = style ? semanticStyle(style) : '';

  for (const attr of Array.from(el.attributes)) {
    if (!KEEP_ATTRS.has(attr.name.toLowerCase())) el.removeAttribute(attr.name);
  }
  if (keptStyle) el.setAttribute('style', keptStyle);

  // 图片只接受内嵌 data:image(外链在 CSP 里本就加载不出来,写进 CF_HTML 也是死链)
  if (el.tagName === 'IMG') {
    const src = (el.getAttribute('src') ?? '').trim();
    if (!/^data:image\//i.test(src)) el.removeAttribute('src');
  }
}

/** 内联样式 → 只保留语义格式的声明串(空串表示整条 style 该丢) */
function semanticStyle(style: CSSStyleDeclaration): string {
  const decls: string[] = [];

  const weight = style.getPropertyValue('font-weight').trim().toLowerCase();
  if (weight === 'bold' || weight === 'bolder') decls.push('font-weight:600');
  else if (/^\d+$/.test(weight) && Number(weight) >= 600) decls.push('font-weight:600');

  const fontStyle = style.getPropertyValue('font-style').trim().toLowerCase();
  if (fontStyle === 'italic' || fontStyle === 'oblique') decls.push('font-style:italic');

  // text-decoration 常以简写出现(underline solid rgb(...)),按 token 判定
  const decoration = (
    style.getPropertyValue('text-decoration-line') ||
    style.getPropertyValue('text-decoration') ||
    ''
  ).toLowerCase();
  const lines: string[] = [];
  if (decoration.includes('underline')) lines.push('underline');
  if (decoration.includes('line-through')) lines.push('line-through');
  if (lines.length) decls.push(`text-decoration:${lines.join(' ')}`);

  const align = style.getPropertyValue('text-align').trim().toLowerCase();
  if (align === 'left' || align === 'center' || align === 'right') decls.push(`text-align:${align}`);

  const verticalAlign = style.getPropertyValue('vertical-align').trim().toLowerCase();
  if (verticalAlign === 'super' || verticalAlign === 'sub') {
    decls.push(`vertical-align:${verticalAlign}`);
  }

  return decls.join(';');
}

/** 空 span(无文本、无 img/br)是编辑器留下的空壳,存进去只会让 CF_HTML 变脏。
 *  删掉内层可能让外层也变空,所以循环到不再变化。 */
function removeEmptySpans(root: HTMLElement): void {
  for (let changed = true; changed; ) {
    changed = false;
    for (const span of Array.from(root.querySelectorAll('span'))) {
      if ((span.textContent ?? '').trim() !== '') continue;
      if (span.querySelector('img,br')) continue;
      span.remove();
      changed = true;
    }
  }
}
