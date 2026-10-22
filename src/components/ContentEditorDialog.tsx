import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardItem as ClipboardItemType } from '../types/clipboard';
import { buildRichPreviewSrcDoc } from '../lib/richPreview';
import { cleanEditableHtml, extractEditableText } from '../lib/richEditor';
import { CopyIcon, XIcon, CheckIcon } from './icons';

/** 查看 / 编辑条目的完整内容。item 为 null 时不渲染;
 *  仅文本类条目由列表的「眼睛」按钮打开(图片条目不提供入口)。
 *  富文本视图是**所见即所得编辑器**:iframe 开 designMode,保存时把正文
 *  归一成「纯文本 + 语义 HTML」交给后端(update_item_rich_content)。 */
export function ContentEditorDialog({
  item,
  onClose,
}: {
  item: ClipboardItemType | null;
  onClose: () => void;
}) {
  const [text, setText] = useState('');
  const [selection, setSelection] = useState({ start: 0, end: 0 });
  const [autoClose, setAutoClose] = useState(true);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // 富文本预览:后端返回净化后的 CF_HTML 片段。连 id 一起存,
  // 换条目时旧片段立刻失效,不会先渲染上一条的图文再被替换
  const [rich, setRich] = useState<{ id: number; html: string | null } | null>(null);
  const [view, setView] = useState<'rich' | 'plain'>('plain');
  // 富文本 iframe 里的选中文字:与 textarea 的 selection 分开记,
  // 「复制选中」按钮在两种视图下都能亮
  const [richSelected, setRichSelected] = useState('');
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  // 已经挂过监听的 iframe 文档(React 可能重复触发 onLoad,避免重复绑定)
  const boundDocs = useRef(new WeakSet<Document>());
  // iframe 内的监听器只绑一次,回调走 ref 取最新闭包(autoClose/onClose 会变)
  const richHandlers = useRef<{ onClose: () => void; onNativeCopy: () => void }>({
    onClose,
    onNativeCopy: () => {},
  });
  const copyTimer = useRef<number | undefined>(undefined);

  // item 变化(打开新条目)时重置草稿与全部临时状态,避免残留上一条的选区/错误
  useEffect(() => {
    if (!item) return;
    setText(item.content);
    setSelection({ start: 0, end: 0 });
    setCopied(false);
    setError(null);
    setSaving(false);
    setRich(null);
    setRichSelected('');
    // 聚焦但不全选:全选会让用户随手输入就覆盖原内容
    areaRef.current?.focus();

    // 无富格式(html_path 为空)的条目:维持原有的纯文本行为,不显示切换
    if (!item.html_path) {
      setView('plain');
      return;
    }
    // 带富格式:默认打开富文本视图;片段拉不到(文件缺失/守卫拒绝/解析失败)
    // 就退回纯文本,同样不显示切换
    setView('rich');
    let cancelled = false;
    invoke<{ html: string | null }>('get_rich_preview', { id: item.id })
      .then((res) => {
        if (cancelled) return;
        const html = res?.html ?? null;
        setRich({ id: item.id, html });
        if (!html) setView('plain');
      })
      .catch(() => {
        if (cancelled) return;
        setRich({ id: item.id, html: null });
        setView('plain');
      });
    return () => {
      cancelled = true;
    };
  }, [item]);

  // Esc 关闭。挂在 window 上而非面板上,保证焦点在 textarea 内也能命中
  useEffect(() => {
    if (!item) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [item, onClose]);

  useEffect(() => () => window.clearTimeout(copyTimer.current), []);

  const syncSelection = useCallback(() => {
    const el = areaRef.current;
    if (!el) return;
    setSelection({ start: el.selectionStart, end: el.selectionEnd });
  }, []);

  const hasSelection = selection.end > selection.start;

  // 当前条目的富文本片段(带 id 校验:换条目后旧片段立即失效)
  const richHtml = item && rich?.id === item.id ? rich.html : null;
  // 有 html_path 但结果还没回来
  const richLoading = Boolean(item?.html_path) && rich?.id !== item?.id;
  // 只有真拿到片段才显示切换,拿不到就当普通纯文本条目
  const showTabs = richHtml !== null;
  const showRichView = showTabs && view === 'rich';
  // srcDoc 只在片段变化时重建:父组件因输入/选区/保存状态重渲染时不能重建
  // iframe —— 重建会丢掉 designMode 与用户正在编辑的内容
  const richSrcDoc = useMemo(
    () => (richHtml === null ? '' : buildRichPreviewSrcDoc(richHtml)),
    [richHtml],
  );
  // 「复制选中」:纯文本看 textarea 选区,富文本看 iframe 里的选中
  const canCopy = showRichView ? richSelected.trim().length > 0 : hasSelection;

  const handleCopy = async () => {
    let selected = '';
    if (showRichView) {
      // 直接读 iframe 的实时选区,不信 state(selectionchange 有节流延迟)
      selected = iframeRef.current?.contentDocument?.getSelection()?.toString() ?? '';
    } else {
      const el = areaRef.current;
      if (!el) return;
      selected = el.value.slice(el.selectionStart, el.selectionEnd);
    }
    if (!selected) return;
    setError(null);
    try {
      await invoke('write_clipboard_text', { text: selected });
      if (autoClose) {
        onClose();
        return;
      }
      // 不自动关闭时给一次短暂的成功反馈
      setCopied(true);
      window.clearTimeout(copyTimer.current);
      copyTimer.current = window.setTimeout(() => setCopied(false), 1200);
    } catch (err) {
      setError(String(err));
    }
  };

  // Ctrl+C 原生复制同样算「复制选中」:勾选自动关闭时也关窗。
  // 剪贴板写入发生在 copy 事件派发完成之后,故延后一拍再关,
  // 确保复制先落地、关闭后主程序的剪贴板监控才能捕获到新内容
  const handleNativeCopy = useCallback(() => {
    if (autoClose) window.setTimeout(onClose, 0);
  }, [autoClose, onClose]);

  // 监听器只绑一次(见 handleRichLoad),这里把最新闭包塞进 ref
  useEffect(() => {
    richHandlers.current = { onClose, onNativeCopy: handleNativeCopy };
  }, [onClose, handleNativeCopy]);

  /** iframe 载入完成:打开 designMode,把文档变成可编辑的所见即所得区域。
   *  sandbox 只给 allow-same-origin(无 allow-scripts),脚本仍被禁;
   *  同源仅用于父文档设置 designMode 与读回内容。 */
  const handleRichLoad = useCallback(() => {
    const doc = iframeRef.current?.contentDocument;
    if (!doc) return;
    // designMode 比给 body 加 contenteditable 更接近"整页编辑器":
    // 回车成段、列表/标题/链接/图片等原生编辑命令都能用
    doc.designMode = 'on';
    const body = doc.body;
    if (body) {
      body.contentEditable = 'true';
      body.spellcheck = false;
      // 光标颜色交给 richPreview 的 CSS(caret-color),这里只去 focus 外框
      body.style.outline = 'none';
    }
    if (boundDocs.current.has(doc)) return;
    boundDocs.current.add(doc);

    // 选区变化 → 「复制选中」按钮的可用态
    doc.addEventListener('selectionchange', () => {
      setRichSelected(doc.getSelection()?.toString() ?? '');
    });
    // iframe 内的按键不会冒泡到父 window,Esc 必须在这里接
    doc.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        richHandlers.current.onClose();
      }
    });
    // Ctrl+C 同样算「复制选中」:勾选自动关闭时关窗(与 textarea 一致)
    doc.addEventListener('copy', () => richHandlers.current.onNativeCopy());
  }, []);

  /** 切换视图。富文本 → 纯文本:先把 iframe 正文读成纯文本灌进 textarea,
   *  否则纯文本视图显示的是打开时那份旧内容,刚编辑的东西看不见。
   *  纯文本 → 富文本:不动 iframe(它一直挂载着),编辑现场原样保留。 */
  const handleViewChange = (next: 'rich' | 'plain') => {
    if (next === view) return;
    if (next === 'plain') {
      const body = iframeRef.current?.contentDocument?.body;
      if (body) {
        setText(extractEditableText(body));
        setSelection({ start: 0, end: 0 });
      }
    }
    setView(next);
  };

  const handleSave = async () => {
    if (!item || saving) return;
    // 富文本片段还没回来(richLoading):showTabs=false、iframe 未挂载,
    // 此时保存会落到纯文本分支,把条目原有的富文本格式清掉 —— 直接拦住。
    // 放在 setSaving(true) 之前,免得按钮卡在「保存中」
    if (richLoading) {
      setError('正在载入富文本,请稍候');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      // 富文本视图:以 iframe 里的**实际内容**为准(textarea 可能是旧的)
      const body = showRichView ? iframeRef.current?.contentDocument?.body : null;
      if (showRichView && !body) {
        // 极端情况:片段已到手但 iframe 还没就绪(contentDocument 不可访问)。
        // 绝不回退纯文本写入 —— 那会把富文本格式误清掉
        setError('富文本编辑器尚未就绪,请稍候重试');
        setSaving(false);
        return;
      }
      if (body) {
        const html = cleanEditableHtml(body);
        const content = extractEditableText(body);
        // 「内容被清空」按文本 + 图片判定,而不是 html.trim():
        // 浏览器把正文删干净后 body 里仍留着 <div><span><br></span></div>
        // 这类空壳,html.trim() 永远不为空,那条回退分支就形同虚设。
        const emptied = content.trim() === '' && !body.querySelector('img');
        if (emptied) {
          // 退回纯文本写入(清掉格式与 CF_HTML),与原有逻辑一致
          await invoke('update_item_content', { id: item.id, content });
        } else {
          await invoke('update_item_rich_content', { id: item.id, content, html });
        }
      } else {
        await invoke('update_item_content', { id: item.id, content: text });
      }
      onClose();
    } catch (err) {
      setError(String(err));
      setSaving(false);
    }
  };

  if (!item) return null;

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center bg-black/50 clip-fade-in"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="clip-dialog bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-[560px] mx-4 overflow-hidden flex flex-col max-h-[88vh] clip-pop-in">
        <div className="flex items-center justify-between px-5 pt-3.5 pb-3 border-b border-hairline">
          <h3 className="text-sm font-semibold">查看 / 编辑内容</h3>
          <button
            onClick={onClose}
            className="flex items-center justify-center w-[26px] h-[26px] rounded-[7px] text-faint hover:bg-hairline hover:text-muted transition-colors"
            title="关闭"
          >
            <XIcon size={14} />
          </button>
        </div>

        {/* min-h + overflow-y-auto:极矮窗口下 textarea 的 160px 下限可能
            撑破面板 88vh 上限,让内容区自己滚动,保证底部按钮永远可见 */}
        <div className="px-5 py-4 flex-1 min-h-0 overflow-y-auto">
          {showTabs && (
            <div className="flex items-center gap-4 mb-3 border-b border-hairline">
              {(['rich', 'plain'] as const).map((v) => (
                <button
                  key={v}
                  onClick={() => handleViewChange(v)}
                  className={`-mb-px h-[26px] text-[12.5px] whitespace-nowrap border-b-2 transition-colors duration-150 ${
                    view === v
                      ? 'text-accent border-accent'
                      : 'text-faint border-transparent hover:text-muted'
                  }`}
                >
                  {v === 'rich' ? '富文本编辑' : '纯文本'}
                </button>
              ))}
            </div>
          )}
          {richLoading ? (
            <div className="w-full h-[46vh] min-h-[160px] rounded-[10px] border border-hairline bg-app flex items-center justify-center text-[12.5px] text-faint">
              正在载入富文本…
            </div>
          ) : (
            <>
              {/* 富文本 iframe 只要 showTabs 就始终挂载:切到纯文本时仅加 hidden,
                  切回来时编辑内容/选区/designMode 都还在(重建 iframe 会全丢)。
                  sandbox="allow-same-origin" 只给同源(父文档要设 designMode、
                  读回内容),不给 allow-scripts —— 脚本、表单、弹窗、顶层导航仍禁。
                  srcDoc(内联 CSP + 排版归一 + 主题色)与悬停气泡共用
                  lib/richPreview.buildRichPreviewSrcDoc;底色用 bg-surface 跟主题走,
                  载入前后不跳色。绝不把片段 dangerouslySetInnerHTML 进主文档。 */}
              {showTabs && (
                <>
                  <p
                    className={`mb-2 text-[11.5px] text-faint ${showRichView ? '' : 'hidden'}`}
                  >
                    可直接编辑；保存会同步更新富文本格式
                  </p>
                  <iframe
                    ref={iframeRef}
                    title="富文本编辑"
                    sandbox="allow-same-origin"
                    srcDoc={richSrcDoc}
                    onLoad={handleRichLoad}
                    className={`w-full h-[46vh] min-h-[160px] rounded-[10px] border border-hairline bg-surface ${
                      showRichView ? '' : 'hidden'
                    }`}
                  />
                </>
              )}
              {!showRichView && (
                <>
                  {showTabs && (
                    <p className="mb-2 text-[11.5px] text-faint">
                      编辑并保存纯文本将丢弃富文本格式(含图片),粘贴将退化为纯文本
                    </p>
                  )}
                  <textarea
                    ref={areaRef}
                    value={text}
                    onChange={(e) => setText(e.target.value)}
                    onSelect={syncSelection}
                    onKeyUp={syncSelection}
                    onMouseUp={syncSelection}
                    onCopy={handleNativeCopy}
                    className="w-full h-[46vh] min-h-[160px] px-3 py-2 rounded-[10px] border border-hairline bg-app text-[13px] leading-relaxed text-ink outline-none resize-none overflow-y-auto focus:border-accent focus:ring-[3px] focus:ring-accent-ring transition-[border-color,box-shadow] duration-150"
                  />
                </>
              )}
            </>
          )}
          {error && <p className="mt-2 text-[11.5px] text-danger">{error}</p>}
        </div>

        {/* 底部固定两行(选项一行、按钮一行):窄面板下横向一行会被挤到
            每个字竖排折行(实测 370px 面板溢出),按钮一律 nowrap 防折行 */}
        <div className="px-5 pt-3 pb-4 border-t border-hairline">
          <label className="flex items-center gap-2 text-[12.5px] text-muted cursor-pointer select-none w-fit">
            <input
              type="checkbox"
              checked={autoClose}
              onChange={(e) => setAutoClose(e.target.checked)}
              className="sr-only"
            />
            <span
              aria-hidden="true"
              className={`flex items-center justify-center w-[15px] h-[15px] rounded-[4px] border transition-colors duration-150 ${
                autoClose ? 'bg-accent border-accent text-on-accent' : 'border-hairline text-transparent'
              }`}
            >
              <CheckIcon size={11} />
            </span>
            复制选中后自动关闭此窗口
          </label>
          <div className="flex items-center justify-end gap-2 mt-3">
            <button
              onClick={() => void handleCopy()}
              disabled={!canCopy}
              className={`flex items-center gap-1.5 h-[31px] px-3 text-[12.5px] whitespace-nowrap rounded-[10px] transition-colors duration-150 ${
                canCopy
                  ? 'text-muted hover:bg-app'
                  : 'text-faint cursor-not-allowed'
              }`}
            >
              <CopyIcon size={13} />
              {copied ? '已复制' : '复制选中'}
            </button>
            <button
              onClick={onClose}
              className="h-[31px] px-4 text-[12.5px] whitespace-nowrap rounded-[10px] text-muted hover:bg-app transition-colors duration-150"
            >
              取消
            </button>
            <button
              onClick={() => void handleSave()}
              disabled={saving || richLoading}
              className={`h-[31px] px-5 text-[12.5px] whitespace-nowrap rounded-[10px] font-semibold border transition-colors duration-150 ${
                saving || richLoading
                  ? 'bg-accent-soft text-faint border-transparent cursor-not-allowed'
                  : 'bg-accent-soft text-accent border-accent-ring hover:bg-accent hover:text-on-accent'
              }`}
            >
              保存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
