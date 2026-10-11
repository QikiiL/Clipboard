/** 富文本预览 iframe 的 srcDoc 构造器。
 *
 *  弹窗(ContentEditorDialog)与悬停气泡(ClipboardItem)共用同一份,
 *  避免两处样式与 CSP 各自漂移。
 *
 *  安全模型(与后端净化共同构成多层防御):
 *  - 调用方必须用 `sandbox=""` 的 iframe 承载:禁脚本、禁同源、禁表单、
 *    禁弹窗、禁顶层导航;
 *  - 这里的 CSP 再把文档级资源收窄到「只允许内联样式 + data: 内嵌图片」,
 *    外链图片/样式/字体与 meta-refresh 之外的旁路一并堵死;
 *  - 片段本身已由后端 sanitize_clipboard_html 净化过。
 *
 *  样式:字号/行高与纯文本编辑器的 `text-[13px] leading-relaxed` 对齐。
 *  剪贴板富文本普遍**自带字号**——Word/Outlook 片段是
 *  `<span style="font-size:12.0pt">`(12pt≈16px),内联样式优先级高于
 *  样式表里对 body 的普通声明,所以只设 body 字号管不到子元素:必须用
 *  `body *{font-size:inherit !important}` 强制子元素继承 body 的 13px。
 *  line-height 同理(源 HTML 也常带内联 line-height / mso-line-height),
 *  一并 `!important` 才能与纯文本视图逐值一致。
 *  代价:源里的 h1/h2 等标题字号也被压平成 13px —— 预览场景要的就是
 *  "与纯文本一致",可接受。
 *  img 三条规则同样带 `!important` —— 源 HTML 常写死尺寸(Outlook 等):
 *  属性形式(height="300")作者样式本就压得住,但内联样式
 *  (`style="max-width:none"` / `style="height:300px"`)只有 `!important`
 *  才压得过,否则超宽图片仍会撑破布局、超高图片仍会被裁切。 */
export function buildRichPreviewSrcDoc(html: string): string {
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'"><style>html,body{margin:0;padding:8px 10px;font-family:system-ui,"Microsoft YaHei",sans-serif;font-size:13px;line-height:1.625;color:#1a1a1a;background:#fff;overflow-wrap:anywhere}body *{font-size:inherit !important;line-height:inherit !important}img{max-width:100% !important;height:auto !important;max-height:100% !important}</style></head><body>${html}</body></html>`;
}
