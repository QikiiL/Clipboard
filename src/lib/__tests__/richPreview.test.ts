import { describe, it, expect } from 'vitest';
import { buildRichPreviewSrcDoc } from '../richPreview';

// 弹窗(ContentEditorDialog)与悬停气泡(ClipboardItem)共用这一份 srcDoc,
// 所以样式与 CSP 的回归都锁在这里
describe('buildRichPreviewSrcDoc', () => {
  const doc = buildRichPreviewSrcDoc('<p>你好</p>');

  it('embeds the sanitized fragment verbatim', () => {
    expect(doc).toContain('<p>你好</p>');
  });

  it('aligns with the plain-text editor metrics (13px / leading-relaxed)', () => {
    expect(doc).toContain('font-size:13px');
    expect(doc).toContain('line-height:1.625');
  });

  it('forces images to scale down instead of keeping hardcoded sizes', () => {
    // 三条都必须带 !important:源 HTML 的内联 style(max-width:none、
    // height:300px)只有 !important 才压得住,否则超宽图片仍会撑破布局
    expect(doc).toContain('max-width:100% !important');
    expect(doc).toContain('height:auto !important');
    expect(doc).toContain('max-height:100% !important');
  });

  it('overrides per-element font metrics coming from the source HTML', () => {
    // 剪贴板富文本普遍自带内联字号(Word/Outlook 的 12pt≈16px),
    // 子元素必须被强制继承 body 的 13px/1.625,否则字号跟着源走,
    // 与纯文本视图对不齐。正则锚在 body * 规则内,声明顺序无关
    expect(doc).toMatch(/body \*\{[^}]*font-size:inherit !important/);
    expect(doc).toMatch(/body \*\{[^}]*line-height:inherit !important/);
  });

  it('resets the source page colors and layout inside the fragment', () => {
    // 来源页面的内联样式(深色聊天的青蓝底/白字、工具输出的 grid 两列)
    // 必须被压掉:背景透明、文字继承、display 退回 UA 语义、
    // sticky 与内层滚动(max-height/overflow)一并失效
    expect(doc).toMatch(/body \*\{[^}]*background:transparent !important/);
    expect(doc).toMatch(/body \*\{[^}]*color:inherit !important/);
    expect(doc).toMatch(/body \*\{[^}]*display:revert !important/);
    expect(doc).toMatch(/body \*\{[^}]*max-height:none !important/);
    expect(doc).toMatch(/body \*\{[^}]*overflow:visible !important/);
    expect(doc).toMatch(/body \*\{[^}]*position:static !important/);
  });

  it('turns pre-wrap spans/divs into block-level code blocks', () => {
    // 工具输出用 <span style="white-space: pre-wrap"> 模拟代码块;
    // 统一成块级代码块后才不会留内层滚动条与 grid 错位
    expect(doc).toMatch(/\[style\*="pre-wrap"\]\{[^}]*display:block !important/);
    expect(doc).toMatch(/\[style\*="pre-wrap"\]\{[^}]*white-space:pre-wrap !important/);
    expect(doc).toMatch(/\[style\*="pre-wrap"\]\{[^}]*word-break:break-word/);
    // 来源的 sticky 小标签降级成灰色小标题(位置已由 reset 归零)
    expect(doc).toMatch(/\[style\*="position: sticky"\]\{[^}]*display:block !important/);
  });

  it('injects the host theme palette so dark/light both follow the app', () => {
    const themed = buildRichPreviewSrcDoc('<p>你好</p>', {
      surface: '#101014',
      text: '#f2f2f5',
      muted: '#a0a0aa',
      hair: '#2a2a30',
      surface2: '#1c1c22',
      accent: '#7aa2f7',
      sel: '#33334a',
    });
    // palette 落在 :root 变量上,规则只引用 var() → 一处改动全文档生效
    expect(themed).toContain('--rp-surface:#101014');
    expect(themed).toContain('--rp-text:#f2f2f5');
    expect(themed).toContain('background:var(--rp-surface)');
    expect(themed).toContain('color:var(--rp-text)');
  });

  it('falls back to light colors when the theme variables are unreadable', () => {
    // jsdom 的 getComputedStyle 读不到自定义属性 → 默认参数走 fallback,
    // 不会把空值注入成 `background:var(--rp-surface)` 的无效声明
    expect(doc).toContain('--rp-surface:#ffffff');
    expect(doc).toContain('--rp-text:#1a1a1a');
  });

  it('keeps a fragment with inline font-size but still flattens it', () => {
    const withInline = buildRichPreviewSrcDoc('<span style="font-size:12.0pt">正文</span>');
    // 片段原样内嵌(净化在前端之前已完成),压平靠 CSS 规则
    expect(withInline).toContain('<span style="font-size:12.0pt">正文</span>');
    expect(withInline).toMatch(/body \*\{[^}]*font-size:inherit !important/);
  });

  it('wraps overlong words instead of letting them burst the frame', () => {
    expect(doc).toContain('overflow-wrap:anywhere');
  });

  it('keeps the strict CSP: only inline styles and inline data images', () => {
    expect(doc).toContain("default-src 'none'");
    expect(doc).toContain('img-src data:');
    expect(doc).toContain("style-src 'unsafe-inline'");
  });

  it('makes the document fill the frame so blank areas take the caret', () => {
    // 弹窗里 iframe 是 designMode 编辑器:body 撑满 + border-box,
    // 点正文下方的空白区也能落光标(否则只有文字本身可点)
    expect(doc).toContain('html{height:100%}');
    expect(doc).toContain('min-height:100%');
    expect(doc).toContain('box-sizing:border-box');
  });

  it('themes the caret and drops the focus ring in the editable view', () => {
    // 光标色跟主题走;focus 外框去掉(焦点已由 iframe 边框表达)
    expect(doc).toContain('caret-color:var(--rp-accent)');
    expect(doc).toContain('body:focus{outline:none}');
  });
});
