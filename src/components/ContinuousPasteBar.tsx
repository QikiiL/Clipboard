import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '../stores/clipboardStore';
import {
  useContinuousPasteStore,
  type ContinuousStatus,
} from '../stores/continuousPasteStore';
import { CheckIcon, LayersIcon, PlayIcon, StopIcon } from './icons';

const btn =
  'h-[24px] px-2.5 text-[11.5px] rounded-[8px] transition-colors duration-150 whitespace-nowrap flex items-center gap-1 flex-shrink-0';
const btnGhost = `${btn} border border-hairline text-muted hover:bg-hairline`;
const btnPrimary = `${btn} bg-accent text-on-accent font-medium hover:bg-accent-deep disabled:opacity-40 disabled:cursor-not-allowed`;

/**
 * 连续粘贴模式条(挂在 GroupTabs 与列表之间):
 * - 框选阶段:提示 + 已选计数 + 开始/自动连贴/清空/退出;
 * - 执行阶段:剩余进度 + 下一条预览 + 粘贴下一条(或停止自动)+ 停止;
 * - 完成阶段:汇总 + 完成。
 * 队列事实来源在后端,本条所有状态经 `continuous-paste` 事件同步,
 * 窗口销毁重建后依然正确。
 */
export function ContinuousPasteBar() {
  const mode = useContinuousPasteStore((s) => s.mode);
  const status = useContinuousPasteStore((s) => s.status);
  const selectedCount = useContinuousPasteStore((s) => s.selectedIds.size);
  const lastError = useContinuousPasteStore((s) => s.lastError);
  const items = useClipboardStore((s) => s.items);
  const [seqHotkey, setSeqHotkey] = useState('');
  // 完成后短暂展示结果再自动退出模式的定时器
  const finishTimerRef = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(finishTimerRef.current), []);

  // 步进热键展示(仅提示;改设置后回到框选阶段会重新读取)
  useEffect(() => {
    if (!mode) return;
    invoke<{ seq_paste_modifier: string; seq_paste_key: string }>('load_settings')
      .then((s) => setSeqHotkey(`${s.seq_paste_modifier}+${s.seq_paste_key}`))
      .catch(() => {});
  }, [mode]);

  // 后端进度事件 → store(全部状态以事件里的快照为准)
  useEffect(() => {
    const unlisten = listen<{
      event: string;
      status?: ContinuousStatus;
      message?: string;
    }>('continuous-paste', (e) => {
      const store = useContinuousPasteStore.getState();
      if (e.payload.status) store.applyStatus(e.payload.status);
      if (e.payload.event === 'error') {
        store.setLastError(e.payload.message ?? '粘贴失败');
      } else if (e.payload.event !== 'cancelled') {
        store.setLastError(null);
      }
      // 完成:让「已按顺序粘贴 N 条」展示几秒,然后自动退出模式,
      // 列表置灰与横幅随之自动还原,不必手动点「完成」
      if (e.payload.event === 'finished') {
        window.clearTimeout(finishTimerRef.current);
        finishTimerRef.current = window.setTimeout(() => {
          const st = useContinuousPasteStore.getState();
          if (st.mode && st.status && !st.status.active && st.status.total > 0) {
            st.setMode(false);
          }
        }, 3500);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // 键盘:Enter 快捷开始(框选阶段且有选择)。触发条件收紧到焦点在 body 上:
  // 焦点在任何输入框/按钮(搜索框、设置面板、对话框)时不抢键,
  // Escape 也不在此处理 —— 设置面板与各对话框都有既定的 Escape 行为,
  // 两处叠加会一次按键产生两个动作;退出模式请用「退出/完成」按钮
  useEffect(() => {
    if (!mode) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (
        e.key === 'Enter' &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.metaKey &&
        !e.shiftKey &&
        document.activeElement === document.body
      ) {
        const st = useContinuousPasteStore.getState();
        if (!st.status?.active && st.selectedIds.size > 0) {
          e.preventDefault();
          void st.start();
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [mode]);

  if (!mode) return null;

  const queueActive = status?.active ?? false;
  // 队列已取空但未取消:展示完成态(pasted 仍在后端保留)
  const finished = !!status && !status.active && status.total > 0;
  const nextItem =
    status?.next_id != null ? items.find((i) => i.id === status.next_id) : null;
  // 热键提示缀到按钮文本上,窄窗口下省去(见 @max-narrow:hidden)
  const hotkeySuffix = seqHotkey ? (
    <span className="@max-narrow:hidden"> ({seqHotkey})</span>
  ) : null;

  return (
    <div className="px-3 pb-1.5 @min-wide:px-4">
      {/* flex-wrap:窗口窄到放不下「文字 + 按钮组」时,按钮组整体折到第二行,
          组内再放不下继续折 —— 绝不让按钮溢出圆角边框 */}
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5 min-h-9 px-2.5 py-1.5 rounded-[10px] bg-accent-soft ring-1 ring-inset ring-accent-ring text-[12px] min-w-0">
        <LayersIcon size={14} className="text-accent shrink-0" aria-hidden />

        {!queueActive && !finished && (
          <>
            <span className="font-semibold text-accent shrink-0">框选模式</span>
            <span className="text-muted truncate min-w-0 flex-1">
              点击或拖拽框选条目，已选 {selectedCount} 条
            </span>
            <div className="flex flex-wrap items-center justify-end gap-1.5 min-w-0 ml-auto">
              <button onClick={() => useContinuousPasteStore.getState().clearSelection()} className={btnGhost}>
                清空
              </button>
              <button onClick={() => useContinuousPasteStore.getState().setMode(false)} className={btnGhost}>
                退出
              </button>
              <button
                onClick={() => void useContinuousPasteStore.getState().startAuto()}
                disabled={selectedCount === 0}
                className={`${btn} bg-surface shadow-lift text-ink hover:shadow-lift-hover disabled:opacity-40 disabled:cursor-not-allowed`}
                title="按固定间隔自动逐条粘贴到当前输入框"
              >
                <PlayIcon size={11} />
                自动连贴
              </button>
              <button
                onClick={() => void useContinuousPasteStore.getState().start()}
                disabled={selectedCount === 0}
                className={btnPrimary}
                title="隐藏面板并回到目标窗口;每按一次 Ctrl+V 依次粘贴一条,第一条也由按键触发"
              >
                开始连续粘贴{hotkeySuffix}
              </button>
            </div>
          </>
        )}

        {queueActive && status && (
          <>
            <span className="font-semibold text-accent shrink-0">连续粘贴中</span>
            <span className="text-muted shrink-0 tabular-nums">
              剩余 {status.remaining}/{status.total}
            </span>
            <span className="text-muted truncate min-w-0 flex-1">
              {nextItem
                ? status.pasted.length === 0
                  ? // 只武装还没开始:第一条等用户按下触发热键
                    `按 ${seqHotkey || '触发热键'} 开始，第一条：${nextItem.preview || nextItem.content}`
                  : `下一条：${nextItem.preview || nextItem.content}`
                : ''}
            </span>
            <div className="flex flex-wrap items-center justify-end gap-1.5 min-w-0 ml-auto">
              {status.auto_running ? (
                <button
                  onClick={() => void useContinuousPasteStore.getState().stopAuto()}
                  className={`${btn} bg-surface shadow-lift text-ink hover:shadow-lift-hover`}
                >
                  <StopIcon size={11} />
                  停止自动
                </button>
              ) : (
                <button
                  onClick={() => void useContinuousPasteStore.getState().pasteNext()}
                  className={btnPrimary}
                >
                  粘贴下一条{hotkeySuffix}
                </button>
              )}
              <button
                onClick={() => void useContinuousPasteStore.getState().cancel()}
                className={btnGhost}
                title="清空队列并退出连续粘贴"
              >
                停止
              </button>
            </div>
          </>
        )}

        {finished && status && (
          <>
            <CheckIcon size={13} className="text-ok shrink-0" />
            <span className="text-muted truncate min-w-0 flex-1">
              已按顺序粘贴 {status.total} 条，连续粘贴结束
            </span>
            <div className="flex items-center gap-1.5 ml-auto shrink-0">
              <button
                onClick={() => useContinuousPasteStore.getState().setMode(false)}
                className={`${btn} bg-surface shadow-lift text-ink hover:shadow-lift-hover`}
              >
                完成
              </button>
            </div>
          </>
        )}
      </div>

      {lastError && (
        <p className="mt-1 px-1 text-[11px] text-danger">粘贴失败：{lastError}</p>
      )}
      {queueActive && status && !status.hotkey_registered && !status.auto_running && (
        <p className="mt-1 px-1 text-[11px] text-warn-text">
          步进热键不可用（可能与唤出热键冲突或被占用），请使用「粘贴下一条」按钮，或在设置中更换连续粘贴热键
        </p>
      )}
    </div>
  );
}
