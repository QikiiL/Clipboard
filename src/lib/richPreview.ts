/** 富文本预览 iframe 的 srcDoc 构造器。
 *
 *  弹窗(ContentEditorDialog)与悬停气泡(ClipboardItem)共用同一份,
 *  避免两处样式与 CSP 各自漂移。
 *
 *  两种用途:
 *  - 弹窗 = **可编辑**的所见即所得编辑器:iframe 用 `sandbox="allow-same-origin"`,
 *    父文档要访问 contentDocument 打开 designMode、读回编辑后的内容;
 *  - 悬停气泡 = 只读预览:iframe 用 `sandbox=""`(最严),样式与弹窗同一份。
 *  下面第 10 条编辑态样式(撑满高度 + 光标色)只对 designMode 生效,只读预览
 *  里没有光标,不受影响。
 *
 *  安全模型(与后端净化共同构成多层防御):
 *  - 调用方必须用 `sandbox=""` 或 `sandbox="allow-same-origin"` 的 iframe 承载:
 *    两种都不给 `allow-scripts`,脚本一律禁;表单、弹窗、顶层导航同样禁。
 *    `allow-same-origin` 只让「父读子」成立,而父文档是可信的 app 代码;
 *  - 这里的 CSP 再把文档级资源收窄到「只允许内联样式 + data: 内嵌图片」,
 *    外链图片/样式/字体与 meta-refresh 之外的旁路一并堵死;
 *  - 片段本身已由后端 sanitize_clipboard_html 净化过,这里**原样**内嵌,
 *    不做 DOM 重写 —— 删标签/改属性会把安全边界挪到前端,也破坏"片段 verbatim"。
 *
 *  样式策略:保留语义格式,压掉来源页面的**布局与配色噪音**。
 *  剪贴板富文本是从别人页面里抠出来的,内联样式带着那个页面的上下文:
 *  - 深色聊天界面复制来的 h3/p 带 `background-color: rgb(81,173,204)` +
 *    `color: rgb(249,250,251)` —— 在预览里就是一大块青蓝底 + 白字,像误选中;
 *  - 工具输出的 `<div style="display:grid;max-height:150px;overflow-y:auto">`
 *    把 grid 列宽、内层滚动条、sticky 小标签、11px 等宽字一起搬进来,错位又难读。
 *  这些都不是"内容",所以:
 *  1. `body *` 统一 reset(全部 `!important`,压过来源内联样式):background /
 *     color / font-family / font-size / line-height / margin / padding / border
 *     全清;`display:revert` 退回 UA 语义(div 仍是块、table 仍是表、span 仍是
 *     行内);`position:static` 拆 sticky;`max-height:none` / `overflow:visible`
 *     拆内层滚动;宽高、浮动、transform、opacity 一并归零;
 *  2. 之后按 h1–h6 / p,div / ul,ol / li / a / code / pre / blockquote / table /
 *     img / hr / mark 逐条给回**我们自己的**排版:正文 13px/1.625、标题分级、
 *     行内 code 小胶囊、`pre` 与「带 `white-space: pre-wrap` 的 span/div」统一
 *     成块级代码块 —— 样本 2 的「输入/输出」于是变成小标签 + 独立代码块,
 *     内层滚动条与 grid 错位一并消失;
 *  3. 配色不写死:从宿主 `html` 的 CSS 变量读一份 palette 注入 iframe,
 *     浅/深色、浮笺/瑞士/雾绿自动跟随;读不到(jsdom / 首帧)走浅色 fallback。
 *
 *  为什么非 `!important` 不可:源内联样式优先级高于样式表普通声明(Word/Outlook
 *  的 12pt≈16px、深色页面的 color/background),不加就压不住,预览会跟着来源走。
 *  个别来源内联的 `!important`(如 `<code style="font-size:0.875em !important">`)
 *  仍会胜出 —— 内联 !important 在层叠里高于作者样式表,留它无害。
 */

/** iframe 内使用的配色。字段对应宿主 `html` 上的同名 CSS 变量。 */
export interface RichPreviewPalette {
  /** --surface:预览底色 */
  surface: string;
  /** --text:正文色 */
  text: string;
  /** --muted:次要文字(来源小标签等) */
  muted: string;
  /** --hair:细边框 / 滚动条滑块 */
  hair: string;
  /** --surface-2:行内 code 与代码块底色 */
  surface2: string;
  /** --accent:链接色 */
  accent: string;
  /** --sel:选中态 / mark 底色 */
  sel: string;
}

/** 变量读不到时(jsdom、首帧、非浏览器环境)的浅色兜底,与 :root 浅色令牌同调。 */
const FALLBACK_PALETTE: RichPreviewPalette = {
  surface: '#ffffff',
  text: '#1a1a1a',
  muted: '#6b6b73',
  hair: '#e6e6e9',
  surface2: '#f4f4f6',
  accent: '#2f62c8',
  sel: '#dde6fb',
};

/** palette 字段 → 宿主 CSS 变量名 */
const PALETTE_VARS: Record<keyof RichPreviewPalette, string> = {
  surface: '--surface',
  text: '--text',
  muted: '--muted',
  hair: '--hair',
  surface2: '--surface-2',
  accent: '--accent',
  sel: '--sel',
};

/** 从 `document.documentElement` 的计算样式读主题色。
 *  读不到(空串)就逐项回落 —— jsdom 里 `getPropertyValue` 恒为空,
 *  测试与首帧都不会拿到空值。 */
export function readRichPreviewPalette(): RichPreviewPalette {
  const palette: RichPreviewPalette = { ...FALLBACK_PALETTE };
  if (typeof window === 'undefined' || typeof document === 'undefined') return palette;
  let computed: CSSStyleDeclaration | null = null;
  try {
    computed = window.getComputedStyle(document.documentElement);
  } catch {
    return palette;
  }
  if (!computed) return palette;
  for (const key of Object.keys(PALETTE_VARS) as (keyof RichPreviewPalette)[]) {
    const value = computed.getPropertyValue(PALETTE_VARS[key]).trim();
    // 只接受"像颜色"的值:挡掉空串与可能带出声明/规则边界的字符
    if (value && !/[;{}<>]/.test(value)) palette[key] = value;
  }
  return palette;
}

/** 等宽字体栈:与来源页面无关,统一用系统等宽(中文回落雅黑,避免方框) */
const MONO =
  '"SF Mono","JetBrains Mono","Fira Code",Consolas,"Liberation Mono",Menlo,"Microsoft YaHei",monospace';

export function buildRichPreviewSrcDoc(
  html: string,
  palette: RichPreviewPalette = readRichPreviewPalette(),
): string {
  const p = { ...FALLBACK_PALETTE, ...palette };
  const css = `
:root{--rp-surface:${p.surface};--rp-text:${p.text};--rp-muted:${p.muted};--rp-hair:${p.hair};--rp-surface-2:${p.surface2};--rp-accent:${p.accent};--rp-sel:${p.sel}}
html,body{margin:0;padding:0;font-family:system-ui,"Microsoft YaHei",sans-serif;font-size:13px;line-height:1.625;color:var(--rp-text);background:var(--rp-surface)}
body{padding:12px 14px;overflow-x:hidden;overflow-wrap:anywhere}
/* 1. 统一 reset:压掉来源的配色与布局噪音 */
body *{box-sizing:border-box;background:transparent !important;color:inherit !important;font-family:inherit !important;font-size:inherit !important;line-height:inherit !important;margin:0 !important;padding:0 !important;border:0 !important;max-height:none !important;overflow:visible !important;position:static !important;float:none !important;display:revert !important;width:auto !important;height:auto !important;min-width:0 !important;text-indent:0 !important;letter-spacing:normal !important;word-spacing:normal !important;transform:none !important;opacity:1 !important;visibility:visible !important}
/* 2. 语义排版:标题分级 */
h1,h2,h3,h4,h5,h6{font-weight:600 !important;line-height:1.35 !important;margin:14px 0 8px !important;color:var(--rp-text) !important}
h1{font-size:18px !important}
h2{font-size:16px !important}
h3{font-size:14.5px !important}
h4{font-size:13.5px !important}
h5,h6{font-size:13px !important}
/* 首尾不留空隙:来源常带 32px 上边距,预览里就是一片空白 */
body :first-child{margin-top:0 !important}
body :last-child{margin-bottom:0 !important}
p,div{margin:0 0 9px !important}
ul,ol{margin:0 0 9px !important;padding-left:20px !important}
li{margin:0 0 3px !important}
blockquote{margin:0 0 9px !important;padding:0 0 0 10px !important;border-left:3px solid var(--rp-hair) !important;color:var(--rp-muted) !important}
hr{border:0 !important;border-top:1px solid var(--rp-hair) !important;margin:12px 0 !important;height:0 !important}
strong,b{font-weight:600 !important}
em,i{font-style:italic !important}
a{color:var(--rp-accent) !important;text-decoration:underline !important;text-underline-offset:2px}
mark{background:var(--rp-sel) !important;color:inherit !important;padding:0 2px !important;border-radius:3px !important}
/* 3. 行内 code:小胶囊 */
code{font-family:${MONO} !important;font-size:12px !important;line-height:1.6 !important;background:var(--rp-surface-2) !important;border:1px solid var(--rp-hair) !important;border-radius:5px !important;padding:1px 5px !important;white-space:pre-wrap !important;word-break:break-word}
/* 4. 代码块:pre,以及来源里靠 white-space:pre-wrap 模拟的"代码块"
      (工具输出常写成 <span style="white-space: pre-wrap">…</span>)。
      统一成块级:内层滚动、grid 列宽、max-height 截断全部失效 */
pre,[style*="pre-wrap"]{display:block !important;font-family:${MONO} !important;font-size:12px !important;line-height:1.65 !important;background:var(--rp-surface-2) !important;border:1px solid var(--rp-hair) !important;border-radius:8px !important;padding:10px 12px !important;margin:6px 0 10px !important;white-space:pre-wrap !important;word-break:break-word}
pre code{background:transparent !important;border:0 !important;border-radius:0 !important;padding:0 !important;font-size:inherit !important;white-space:inherit !important}
/* 5. 来源的 sticky 小标签(如"输入/输出"):降级成灰色小标题 */
[style*="position: sticky"]{display:block !important;font-size:11.5px !important;line-height:1.5 !important;color:var(--rp-muted) !important;margin:8px 0 4px !important;white-space:normal !important}
/* 6. 纯装饰的空元素(分隔线 span 等)不占位 */
span:empty{display:none !important}
/* 7. 表格:细边框 */
table{border-collapse:collapse !important;width:auto !important;max-width:100% !important;margin:0 0 9px !important}
th,td{border:1px solid var(--rp-hair) !important;padding:4px 8px !important;text-align:left !important;vertical-align:top !important}
th{background:var(--rp-surface-2) !important;font-weight:600 !important}
caption{color:var(--rp-muted) !important;font-size:12px !important;padding:0 0 4px !important}
/* 8. 图片:源 HTML 常写死尺寸(Outlook 的 height="300"、内联
      style="max-width:none"/"height:300px"),三条都带 !important 才压得住,
      否则超宽图撑破布局、超高图被裁切 */
img{max-width:100% !important;height:auto !important;max-height:100% !important;border-radius:6px !important;display:inline-block !important}
::selection{background:var(--rp-sel) !important;color:inherit !important}
/* 9. 细滚动条(内容超长时) */
*{scrollbar-width:thin;scrollbar-color:var(--rp-hair) transparent}
*::-webkit-scrollbar{width:8px;height:8px}
*::-webkit-scrollbar-track{background:transparent}
*::-webkit-scrollbar-thumb{background:var(--rp-hair);border-radius:4px}
/* 10. 编辑态(弹窗 designMode):body 撑满 iframe,点正文下方的空白也能落光标;
      光标色跟主题;focus 外框去掉(焦点已由 iframe 边框表达)。
      只读预览里没有光标与焦点,这几条不改变观感 */
html{height:100%}
body{min-height:100%;box-sizing:border-box;caret-color:var(--rp-accent)}
body:focus{outline:none}
`.replace(/\n\s*/g, '');

  return `<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'"><style>${css}</style></head><body>${html}</body></html>`;
}
