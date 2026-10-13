# 三风格可切换（浮笺 / 瑞士索引 / 雾绿工作台）· 实施方案

> **给执行代理：** REQUIRED SUB-SKILL：`superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`；
> 步骤用 `- [ ]` 跟踪，**每个提交（Task）派一个独立子代理**，Task 3 与 Task 6 结束必须停下请用户验收（观感类改动一次只上一个风格）。
> **本文只写方案，不实现代码。**
>
> **权威视觉来源（值逐字照抄，不重新设计）：** `docs/superpowers/previews/clipboard-style-variants/index.html`（提交 68bc787）——
> §③ 六组令牌块（行 42–48）、§⑤ A/C 组件差异 CSS（行 132–175）、§⑥ 容器查询（行 178–201）。
> 参考：`docs/superpowers/specs/2026-10-15-clipboard-style-variants-design.md` §4.1 令牌、§10 验收。

**Goal:** 在现有「方案 D · 浮笺」之上接入 A 瑞士索引与 C 雾绿工作台，三风格 × 浅深 = 6 态即选即生效；
浮笺保持默认，**在未设置 `data-style` 时视觉与当前 master 完全一致**（向后兼容 = 老用户无感知）。

**架构:** 令牌值全部放 `src/styles/globals.css`，用 `html[data-style="swiss"|"mist"]`（+ `.dark`）属性选择器分层；
风格状态放 `ThemeContext`（不新建 Context、不进 `AppSettings`）；组件只加**语义 hook class / data 属性**（约 12 处一词级改动），
A/C 的结构性差异（行高 52/60、34px 序号栏、页签直角/圆角、圆角 2px/14px）**一律由 CSS 覆盖，组件不做 `style === 'a' ? …` 分支**。

**硬边界:** 不改 `types/settings.ts` / `src-tauri/` / `package.json`（不新增依赖）；不新增 `tailwind.config.js` 内容
（该文件存在但 **Tailwind v4 经 `@tailwindcss/vite` 加载、CSS 里无 `@config` 指令 → 此文件当前根本没被读取**，
主题只走 `globals.css` 的 `@theme inline`；也不要顺手删它，走单独提交）；
不改 `@custom-variant dark` 与容器查询断点（`--container-narrow/wide`）；不动 `App.tsx` 里 `@container` 的挂载位置。

---

## 1. 令牌层（`src/styles/globals.css`）

### 1.1 命名与选择器契约（一次定死，后面都引用它）

| 概念 | 值 | 载体 |
|---|---|---|
| 风格 id | `fuji`（浮笺）/ `swiss`（A 瑞士索引）/ `mist`（C 雾绿工作台） | `localStorage['ui-style']` |
| DOM 标记 | `document.documentElement.dataset.style` | `<html data-style="swiss">` |
| 浅深 | 沿用现有 `theme: 'light' \| 'dark'` | `<html class="dark">` |
| 风格属性选择器 | 浅 `html[data-style="swiss"]`（0,1,1）／深 `html[data-style="swiss"].dark`（0,2,1） | 两者都高于现有 `.dark`（0,1,0）与 `:root`（0,1,0） |

**为什么必须写成 `html[data-style="x"].dark` 而不是 preview 的 `[data-variant="x"][data-theme="dark"]`：**
现有深色是 `.dark`（0,1,0），若深色块也写成 0,1,1，则**浅色块与深色块同分**，命中顺序由源码位置决定 → 脆弱。
写成 0,2,1 后深色块恒胜；且深色块会同时匹配浅色块，只覆盖自己声明的属性，不冲突。

**浮笺默认怎么成立：** `:root` 保持浮笺令牌不变（无属性时也成立，覆盖"React effect 生效前的首帧"）；
`ThemeProvider` 仍然**总是**写 `data-style`（含 `fuji`），便于调试与选择器书写；A/C 之外不新增任何组件差异规则。

### 1.2 文件组织顺序（globals.css 顶部按此重排）

```css
/* ① 浮笺 = 默认令牌（现有 :root 原样保留 + 追加 5 个新令牌） */
:root { /* …现有 26 个… */ --radius: 10px; --surface-2: oklch(95.5% 0.004 260);
        --hover: oklch(96.5% 0.003 260); --sel: oklch(94.5% 0.028 258); --sel-paste: oklch(90% 0.05 258); }
.dark { /* …现有原样… */ --surface-2: oklch(28% 0.008 260); --hover: oklch(27% 0.008 260);
        --sel: oklch(30% 0.055 258); --sel-paste: oklch(34% 0.07 258); }

/* ② A 瑞士索引 · 浅 / 深 —— 值照抄 preview 行 42 / 44，逐字不重算 */
html[data-style="swiss"]      { --bg:oklch(96.2% 0.003 95); --surface:oklch(99.2% 0.002 95); /* …preview 42 全部… */ --radius:2px }
html[data-style="swiss"].dark { --bg:oklch(16.5% 0.004 260); /* …preview 44 全部… */ --radius:2px }

/* ③ C 雾绿工作台 · 浅 / 深 —— 照抄 preview 行 46 / 48 */
html[data-style="mist"]       { --bg:oklch(95.8% 0.008 150); /* …preview 46… */ --radius:14px }
html[data-style="mist"].dark  { --bg:oklch(21% 0.008 165);   /* …preview 48… */ --radius:14px }

@theme inline { /* 现有映射不动，仅追加 4 条 */ --color-surface-2: var(--surface-2);
  --color-hover: var(--hover); --color-sel: var(--sel); --color-sel-paste: var(--sel-paste); }

/* ④ 组件差异：先 A 段、再 C 段（§3.4），全部写成 html[data-style=…] 前缀 */
```

**`--radius` 不放进 `@theme`**（避免和 Tailwind v4 的 `--radius-*` 命名空间/`rounded-*` 工具类混淆），
只作为普通自定义属性；A/C 规则里用 `var(--radius)`。浮笺规则**不引用** `--radius`，所以它的取值（10px = 现有行/搜索框圆角）不影响现状。

### 1.3 浮笺补齐新令牌：取值策略（关键取舍）

preview 新增的 `--hover/--sel/--sel-paste` 与组件需要的 `--surface-2`，浮笺必须有值（否则 `var()` 解析失败 →
`border-radius` 等属性回落 `unset`，会静默变成 0 圆角）。**做法：给浮笺填"等价值"，但浮笺自己的规则一个都不引用它们**，
只有 `html[data-style="swiss"|"mist"]` 段引用 → 取值哪怕不完美也不可能造成浮笺视觉回归。
浮笺等价值：`--surface-2` ≈ 介于 `--surface` 与 `--hair` 的中性面；`--sel` = 现有 `--accent-soft`；`--sel-paste` = 略深的 accent 面；
`--hover` = 现有 `hover:bg-surface` 的近似色（浅色下比 surface 略深一档）。

### 1.4 A/C 令牌块的缺失项审计（**必须逐条决策，不能靠"继承浮笺"糊过去**）

preview 的 A/C 块只声明了面板用得到的属性，以下令牌**在 A/C 下会静默继承浮笺值**，落地时要显式补：
`--ok`、`--warn`、`--warn-text`、`--danger-deep`、`--accent-ring`、`--on-accent`、`--lift`、`--lift-hover`、
`--dialog-shadow`、`--surface-2`、`--hover`、`--sel`、`--sel-paste`、`--radius`，以及 **C 缺 `--accent-deep`**（A 有）。
建议：能沿用浮笺语义的（`--ok/--warn/--danger-deep/--on-accent`）直接继承，**在块内不写**；
但 `--accent-ring`（A 的朱红环）、`--lift/--lift-hover`（A 明确"无阴影"→ 设 `none`，C 用柔和投影）、
`--surface-2`（A 的选中页签底色）必须显式覆盖，否则 A 会出现"朱红底 + 蓝色焦点环"这类串色。
`--dialog-shadow` 在两套里都保留浮笺值即可（弹窗仍要浮起）。

---

## 2. 切换机制：扩展 `ThemeContext`（不新建 StyleContext）

**决策：扩展现有 `src/contexts/ThemeContext.tsx`，文件名与 `ThemeProvider` 名不变**（避免 `App.tsx` 与
`SettingsPanel.tsx` 的 import 改动扩散）。理由：`theme` 与 `style` 写的是**同一个 DOM 根节点**（`documentElement`），
一个 effect 里一起写最省事；新建 StyleContext 会多一层 Provider + 第二个 effect，且两者都要在首帧前对齐，收益为负。
导出仍是 `useTheme()`，返回值增加 `style` / `setStyle`。

```tsx
export type UiStyle = 'fuji' | 'swiss' | 'mist';
const STYLE_KEY = 'ui-style';
// src/lib/uiStyle.ts（纯函数，可单测）
export const UI_STYLES = [
  { value: 'fuji',  label: '浮笺' },
  { value: 'swiss', label: '瑞士索引' },
  { value: 'mist',  label: '雾绿工作台' },
] as const;
export function normalizeStyle(v: string | null): UiStyle {
  return v === 'swiss' || v === 'mist' ? v : 'fuji';   // 未知/缺失 → 浮笺（老用户零迁移）
}
export function readStoredStyle(): UiStyle { return normalizeStyle(localStorage.getItem(STYLE_KEY)); }

// ThemeProvider 内
const [style, setStyle] = useState<UiStyle>(readStoredStyle);
useEffect(() => {
  const root = document.documentElement;
  root.classList.toggle('dark', theme === 'dark');   // 等价改写现有 if/else，行为不变
  root.dataset.style = style;                        // 含 fuji，恒写
  localStorage.setItem('theme', theme);
  localStorage.setItem(STYLE_KEY, style);
}, [theme, style]);
const value = useMemo(() => ({ theme, setTheme, toggleTheme, style, setStyle }), [theme, toggleTheme, style]);
```

**为什么不进 `AppSettings`（HANDOVER §5 / CLAUDE.md 教训）：** 设置面板是"即改即存、整体回传"，
`save_settings` 会拿整份 `AppSettings` 覆盖 store，历史上已因此清空过 `window-state.json` 之类的未知字段；
且风格是**纯前端渲染偏好**，后端没有任何消费点，加进 Rust struct 只会多两处必须同步的类型（HANDOVER §4.3）。
与 `theme` 完全同类 → 同样放 `localStorage`，跨"隐藏即销毁"存活（重建后 mount 时读回）。

**已知取舍（记为风险 R5）：** 现有 `theme` 就是首帧后才由 effect 应用的，深色模式因此有一帧浅色闪变；
风格会叠加同样的闪变（浮笺配色闪一下再变 A/C）。修法见 Task 7（`index.html` 预置 3 行内联脚本），**本方案不强制**。

---

## 3. 组件差异落地

### 3.1 方案选择

- **采用：CSS 属性选择器 + 少量语义 hook class / data 属性。** 组件里零条件逻辑，A/C 差异全部集中在 `globals.css` 的
  `html[data-style=…]` 段；出错只需删/改一条 CSS，回落浮笺样式，不用回滚 JSX。
- **拒绝：`className={style === 'a' ? … : …}` 分支。** 会把 6 处组件的 class 字符串翻倍，且 `memo` 的
  `ClipboardItemCard` 要新增订阅（风格切换会让整个虚拟列表重渲染），侵入与收益不成比例。
- **拒绝：纯结构选择器**（`header`、`.tabs-scroll > button`、`.group\\/…`）——Tailwind 任意值类名带转义/哈希，脆弱且不可读。

### 3.2 preview 选择器 → 真实组件映射（hook 清单，全部是一词级改动）

| preview | 真实位置 | 改动 |
|---|---|---|
| `.panel` | `App.tsx:167` 根 div | + `clip-panel` |
| `.titlebar` | `App.tsx:168` `<header>` | + `clip-titlebar` |
| `.searchrow` / `.searchbox` | `SearchBar.tsx:42` / `:43` | + `clip-searchrow` / `clip-searchbox` |
| `.tabs` / `.tab` | `GroupTabs.tsx:65` / `:8-11` | + `clip-tabs`；页签 + `data-selected={…}` |
| `.list` | `ClipboardList.tsx:153` 滚动容器 | + `clip-list` |
| 行包裹层（无对应） | `ClipboardList.tsx:173`（`pb-[3px]`） | + `clip-row-wrap` |
| `.row` | `ClipboardItem.tsx:342` 根 div | + `clip-row`、`data-selected`、`data-pasted` |
| `[data-role="idx"]` | **新增** span | `clip-idx`（`aria-hidden`） |
| `[data-role="ico"]` | `ClipboardItem.tsx:360` 图标列 | + `clip-ico` |
| `[data-role="acts"]` | `ClipboardItem.tsx:399`（宽度动画容器）/ `:400`（按钮组） | + `clip-actions-wrap` / `clip-actions` |
| `.statusbar` | `StatusBar.tsx:148` 根 div | + `clip-statusbar` |
| `.empty` | `ClipboardList.tsx:140` 空态容器 | + `clip-empty` |

`settings-controls.tsx` 的 `Card`/`Row`/`ghostBtn` **不加 hook**：设置面板在 A/C 下的直角/圆角通过
`html[data-style="swiss"] .clip-dialog …` 这类父级选择器覆盖（优先级 0,1,2 已够压住工具类），
代价是选择器偏宽——验收时重点看弹窗与按钮（见 §5 人工清单）。

### 3.3 现有硬编码冲突清单（逐条已确认，行号为当前 master）

| 位置 | 现状 | A 要求 | C 要求 | 处理 |
|---|---|---|---|---|
| `ClipboardItem.tsx:342` | `min-h-[52px] p-[10px] rounded-[10px] border border-transparent hover:bg-surface hover:border-hairline` + `@max-narrow:*`/`@min-wide:min-h-[60px] p-3` | 52px、`padding:0 10px`、`border-radius:0`、只留 1px 下边线、hover=`--hover` | 60px、`0 12px`、`radius:14px`、`bg-surface` + 柔和投影、hover=`--hover` | CSS 覆盖（0,2,0 > 工具类 0,1,0，**含容器查询内的工具类**，因为容器查询不改变优先级） |
| `ClipboardItem.tsx:342` | `hover:bg-surface` / `border-transparent` | 需 `background:var(--hover)` 且 `border-width:0 0 1px` | 需 `background:var(--surface)`，**border 全去** | 同上，注意 `border-width` 要整条重写 |
| `ClipboardItem.tsx:343` | `ring-2 ring-accent bg-accent-soft/50`（框选选中） | `background:var(--sel)`，序号转 accent | `background:var(--sel)`，标题加粗 | 加 `data-selected`，**整条 `box-shadow` 覆写**（ring 由 `--tw-ring-shadow` 参与合成，逐个清变量不可靠） |
| `ClipboardItem.tsx:344` | `opacity-50`（已粘贴） | `background:var(--sel-paste)` + `inset 0 -2px 0 var(--accent)` | `background:var(--sel-paste)` + `0 0 0 2px var(--accent)` | 加 `data-pasted` |
| `ClipboardItem.tsx:360` | 图标列固定 `w-[26px]` | 序号 34px + 图标 16px | 序号窄 + 图标 16px | `clip-ico{width:16px}`（A/C 段） |
| `ClipboardItem.tsx:399-400` | `pl-2 w-0 overflow-hidden group-hover:w-[118px] transition-[width]`，内层 `opacity-0 group-hover:opacity-100` | 绝对定位覆盖 + `box-shadow:0 0 0 5px var(--bg)`，无动画 | 同 A，底色换 `--surface` | **最大风险点 R1**，见 §6.1；先按 §3.4 覆盖，若点不动/压字则退到 JSX 微调 |
| `ClipboardItem.tsx:407-432` | 操作按钮 `rounded-[7px]` | `2px` | `14px` | A/C 段统一 `border-radius:var(--radius)`（C 的 20px 按钮上 14px≈圆形，**验收时确认是否符合预期**） |
| `ClipboardList.tsx:153` | `px-3 pt-1 pb-2.5 @max-narrow:px-2 @min-wide:px-4` | `padding:0`（分隔线必须通到面板边缘） | `4px 12px 2px` | `clip-list` |
| `ClipboardList.tsx:173` | `pb-[3px]`（行间隙） | `0` | `12px` | `clip-row-wrap` |
| `ClipboardList.tsx:37` | `estimateSize: () => 55` | 52 | 60+12=72 | **R2**：按风格取常数 + 切换后 `virtualizer.measure()` |
| `GroupTabs.tsx:65` | `rounded-full bg-surface border border-hairline p-[3px] gap-[3px]` | 直角、无底无框、`gap:0`、只留下边线 | 无框无底、`gap:6px; padding:2px 12px 8px` | `clip-tabs` |
| `GroupTabs.tsx:8-11` | `tabBase rounded-full px-[13px] py-[5.5px]`；`tabActive = bg-accent-soft text-accent font-semibold` | `radius:0`、`padding:8px 14px`、选中 `bg:--surface-2` + 底线 accent | `radius:999px`、`padding:4px 12px`、选中 `bg:--sel` + `text-ink` | `[data-selected="true"]` 选择器（**不要用 `aria-selected`**：容器不是 tablist，属性会构成无效 ARIA） |
| `SearchBar.tsx:42-43` | `px-4 pt-3 pb-2.5`；`h-9 rounded-[10px] bg-surface border border-hairline focus-within:ring-[3px]` | `padding:0 10px`、`h:22px`、无底无框、只留下边线，focus 换下边色 | `padding:0 12px`、`h:28px`、`radius:999px`、`bg-surface` + 柔和投影、focus `box-shadow:0 0 0 2px var(--accent-soft)` | `clip-searchrow` / `clip-searchbox` |
| `App.tsx:170` | header `border-b border-hairline`、`h-[46px] @min-wide:h-[52px]` | 保留下边线 + `letter-spacing:.06em`（品牌） | **去掉下边线** | `clip-titlebar` |
| `StatusBar.tsx:148` | `h-9 px-4 border-t border-hairline` | 保留上边线、`font-size:10px`、`letter-spacing:.08em` | 去掉上边线、`padding:0 12px` | `clip-statusbar` |
| `ContinuousPasteBar.tsx:117` | `rounded-[10px] bg-accent-soft ring-1 ring-inset ring-accent-ring` | 直角 | 14px | A/C 段覆盖 `border-radius`（其余保留） |
| `Dialogs.tsx:5` / `ContentEditorDialog.tsx:145` / `SettingsPanel.tsx:411` | 弹窗 `rounded-[14px]`；`Card` `rounded-[13px]` | `2px` | 保持 14px（≈现状） | A 段：`html[data-style="swiss"] .clip-dialog, … [role] 容器 { border-radius:2px }`（弹窗加 `clip-dialog` hook 更稳） |

### 3.4 关键覆盖片段（骨架，非全部实现；值照抄 preview §⑤）

```css
/* A · 行：通宽分隔线 / 直角 / 52px / 悬停用 --hover */
html[data-style="swiss"] .clip-row{
  min-height:52px; padding:0 10px; border-radius:0; line-height:1.45;
  background:transparent; border-width:0 0 1px; border-color:var(--hair); box-shadow:none;
}
html[data-style="swiss"] .clip-row:hover{ background:var(--hover); border-color:var(--hair) }
html[data-style="swiss"] .clip-row[data-selected]{ background:var(--sel) }
html[data-style="swiss"] .clip-row[data-selected] .clip-idx{ color:var(--accent); font-weight:600 }
html[data-style="swiss"] .clip-row[data-pasted]{ background:var(--sel-paste); box-shadow:inset 0 -2px 0 var(--accent) }

/* C · 行：卡片化 / 14px / 60px / 12px 间隙 / 明暗分层 */
html[data-style="mist"] .clip-row{
  min-height:60px; padding:0 12px; border-radius:14px; line-height:1.7;
  background:var(--surface); border-color:transparent; box-shadow:0 1px 2px oklch(60% 0.02 165 / .07);
}
html[data-style="mist"] .clip-row:hover{ background:var(--hover); box-shadow:0 1px 3px oklch(60% 0.02 165 / .16) }
html[data-style="mist"] .clip-row[data-selected]{ background:var(--sel) }
html[data-style="mist"] .clip-row[data-pasted]{ background:var(--sel-paste); box-shadow:0 0 0 2px var(--accent) }
html[data-style="mist"].dark .clip-row{ box-shadow:inset 0 0 0 1px var(--hair) }

/* 序号栏：默认隐藏（= 浮笺现状），A 34px 等宽数字，C 窄号 */
.clip-idx{ display:none }
html[data-style="swiss"] .clip-idx{ display:block; width:34px; text-align:right; font-size:12px;
  letter-spacing:.06em; font-variant-numeric:tabular-nums; color:var(--faint) }
html[data-style="mist"] .clip-idx{ display:block; min-width:16px; text-align:right; font-size:12px; color:var(--faint) }

/* 悬停操作：从"宽度动画"改为"绝对定位覆盖"（A/C 共用骨架，仅底色不同）—— R1 */
html[data-style="swiss"] .clip-actions-wrap,
html[data-style="mist"]  .clip-actions-wrap{ width:auto; padding-left:0; overflow:visible; transition:none }
html[data-style="swiss"] .clip-actions,
html[data-style="mist"]  .clip-actions{
  position:absolute; right:10px; top:50%; transform:translateY(-50%);
  gap:2px; padding:1px 3px; border-radius:var(--radius); opacity:0;
  background:var(--bg); box-shadow:0 0 0 5px var(--bg);
}
html[data-style="mist"] .clip-actions{ right:12px; background:var(--surface); box-shadow:0 0 0 5px var(--surface) }
html[data-style="swiss"] .clip-row:hover .clip-actions,
html[data-style="mist"]  .clip-row:hover .clip-actions{ opacity:1 }
```

页签（A 直角分段 / C pill）、搜索框（A 下划线 / C 胶囊）、状态栏、空态（A 32px / C 64px padding + `line-height:1.8`）、
容器查询两档（`@container (max-width:360px)` / `(min-width:520px)`，preview 行 178–201）照 preview §⑤⑥ 平移，
选择器前缀换成 `html[data-style=…]`，**不改** `--container-narrow/wide` 的取值。

### 3.5 需要动 JSX 的 4 处（除 hook 之外）

1. `ClipboardItem.tsx`：新增 `index: number` prop → 渲染 `<span className="clip-idx" aria-hidden="true">{String(index).padStart(2,'0')}</span>`；
   根节点加 `data-selected={boxSelected || undefined}` / `data-pasted={boxUsed || undefined}`。
2. `ClipboardList.tsx`：`<ClipboardItemCard index={virtualRow.index + 1} … />`；
   `estimateSize` 改为按当前风格取 `{ fuji: 55, swiss: 52, mist: 72 }`（读 `document.documentElement.dataset.style`，经 `lib/uiStyle.ts` 的 `rowEstimate()`）；
   新增 effect：`style` 变化时 `virtualizer.measure()`（否则已滚动的长列表行高按旧估计 → 滚动条跳变/错位）。
3. `GroupTabs.tsx`：三个及以上页签按钮加 `data-selected={…}`（复用现有布尔表达式，不加逻辑）。
4. `SearchBar` / `App` / `StatusBar` / `ClipboardList` 空态：仅加 hook class。

---

## 4. 设置 UI（`SettingsPanel.tsx` 外观分区）

复用「关闭行为」那套**整行宽 + 分段控件**的版式（`SettingsPanel.tsx:467-488`），避免 3 个中文标签挤在 `Row` 右侧列里：

```tsx
// 外观 Card 内，深色模式 Row 之后
<div className="px-3.5 py-[11px]">
  <div className="text-[12.5px] font-medium">界面风格</div>
  <p className="mt-[2px] text-[11px] text-faint">即选即生效；浮笺为默认</p>
  <div role="radiogroup" aria-label="界面风格" className="flex mt-2 gap-[3px] p-[3px] rounded-[10px] bg-app">
    {UI_STYLES.map((s) => (
      <button key={s.value} role="radio" aria-checked={style === s.value} onClick={() => setStyle(s.value)}
        className={`flex-1 h-[26px] text-[11.5px] rounded-[7px] transition-[background-color,color,box-shadow] duration-150 ${
          style === s.value ? 'bg-surface text-accent font-semibold shadow-lift' : 'text-muted hover:text-faint'}`}>
        {s.label}
      </button>
    ))}
  </div>
</div>
```

交互契约：`setStyle` 只改 state → effect 写 `dataset` + `localStorage`，**不调 `invoke`**（验证时用 DevTools Network 确认零新请求）；
与深色模式 Switch 同款"即选即生效、无保存按钮"。面板自身跟着风格变（因为选择器挂在 `<html>` 上，弹窗在 `@container` 内不受影响）。

---

## 5. 验证计划

```bash
npx tsc --noEmit      # 必须零错误（build 脚本就是 tsc + vite build）
npx vitest run        # 现有 5 文件全绿 + 新增 src/lib/__tests__/uiStyle.test.ts
npm run dev           # http://localhost:1420（invoke 全失败 → 列表空，但外壳可看）
npm run tauri dev     # 唯一能看到真实行的地方（行级验收必须走这条）
```

**六态矩阵（3 风格 × 浅/深），每态截图存档：** 在 `npm run dev` 下可完整验收的外壳项——
标题栏（A 下边线 / C 无）、品牌字距、搜索框（A 下划线 22px / C 胶囊 28px）、页签（A 直角分段 + accent 底线 / C pill + `--sel`）、
状态栏（A 上边线 + 10px 字距 / C 无 + 12px）、空态、设置弹窗（六分区 + 新风格分段控件 + Switch）、弹窗圆角（A 2px / C 14px）。

**必须在 `npm run tauri dev`（有真实数据）下验收的行级项：** 行高 52 / 60、序号栏（A 34px accent 选中态 / C 窄号）、
通宽分隔线是否通到面板边缘、C 的 12px 行间隙、悬停 4 按钮是否**可点且不压时间文本**、图片缩略图与 meta 行、
收藏/分组图标、连续粘贴框选（`data-selected`）与已粘贴（`data-pasted`）配色、分组右键菜单、内容编辑弹窗、长列表滚动条长度（`estimateSize`）。

**人工点验清单（自动化无法覆盖）：** 逐条点 `ClipboardItem`（悬停→点预览/收藏/分组/删除）、`GroupTabs`（切换 4 个页签 + 右键重命名/删除 + 新建）、
`SearchBar`（聚焦环、清空、Ctrl+F）、`StatusBar`（单击复制/粘贴切换、暂停态圆点）、`SettingsPanel`（新分段控件 3 项 × 深浅 6 次切换、每次切完 F5 刷新确认保持）、
`ContinuousPasteBar`、`Dialogs`/`ContentEditorDialog`。
**不触发的项要显式确认：** 风格切换不产生任何 IPC；`localStorage['ui-style']` 值正确；清掉 `ui-style` 后刷新 = 浮笺且无异常。

---

## 6. 风险与回滚

### 6.1 风险表（按严重度）

| # | 风险 | 影响 | 缓解 |
|---|---|---|---|
| **R1** | **悬停操作按钮**：React 用 `w-0 overflow-hidden group-hover:w-[118px]` 宽度动画，preview 用绝对定位覆盖；覆盖不当 → 按钮点不到（`w-0`+`overflow-hidden`）或压住时间文本 | 交互性回归（比视觉问题严重） | 先按 §3.4 覆盖并在 `tauri dev` 里**逐个点 4 个按钮**；若不稳定，允许把"绝对定位"落到 JSX：给 `clip-actions-wrap` 加 `data-style-dependent` 结构，或仅在 A/C 下渲染成绝对定位容器（3–5 行，无 className 分支） |
| **R2** | `estimateSize: 55` 与 A 52 / C 72 不符；`measure()` 未在切换时调用 | 长列表切换风格后滚动条长度/滚动位置错乱 | Task 4 同时改 `estimateSize` + 切换后 `virtualizer.measure()`；验收用 500+ 条数据滚动到底再切风格 |
| **R3** | `data-style` 写在 `documentElement`，但 A/C 的深色块优先级算错（0,1,1 vs 0,1,0） | 深色下浅色令牌泄漏（如深色底配浅色文字） | 契约固定为 `html[data-style="x"].dark`（0,2,1）；验收必须覆盖 6 态，不能只测浅色 |
| **R4** | A/C 令牌块漏覆盖 `--accent-ring/--lift/--surface-2` 等（§1.4） | 串色（朱红底 + 蓝焦点环）、A 出现不该有的阴影 | 按 §1.4 审计表逐条决策；A 段显式 `--lift:none` |
| **R5** | 首帧闪变（先浮笺配色再切 A/C） | 观感瑕疵 | 可选 Task 7：`index.html` 里 3 行内联脚本，在 React 挂载前读 `theme`/`ui-style` 写 class 与 dataset（同时修掉现有深色闪变） |
| **R6** | C 的 `--radius:14px` 用在 20px 见方的操作按钮上 ≈ 圆形 | 观感与预期不符 | 保留 preview 原值并在验收时给用户看；要改也只改 C 段的 `border-radius` 一处 |
| **R7** | 设置面板/弹窗的选择器偏宽（`clip-dialog` 覆盖） | 误伤其它按钮圆角 | 只覆盖 `border-radius`/`border-color`，不碰尺寸与颜色；弹窗统一加 `clip-dialog` hook 收窄选择器 |

### 6.2 提交策略（干净基线，每步可独立回滚；一支风格一次验收）

| # | Commit | 内容 | 验收 |
|---|---|---|---|
| 1 | `feat(theme): 补齐浮笺 --surface-2/--hover/--sel/--sel-paste/--radius` | 仅 globals.css 增量令牌 + `@theme inline` 4 条映射 | **浮笺视觉零变化**（截图逐像素对比），`tsc`/`vitest` 绿 |
| 2 | `feat(theme): ui-style 存储 + ThemeContext.style + 外观风格分段控件` | `lib/uiStyle.ts`（纯函数 + `rowEstimate()`）、Context 扩展、SettingsPanel 外观分区、`uiStyle.test.ts` | 三选项可切换并持久化；此时只有配色变（组件差异还没上） |
| 3 | `feat(theme): 接入 A/C 六组令牌块` | `html[data-style]` 四块 + §1.4 缺失项 | 六态配色正确（截图）→ **停下请用户验收** |
| 4 | `feat(theme): 组件语义 hook + 序号栏 + 选中/已粘贴 data 属性 + 行高估计` | §3.2 hook 清单、§3.5 四处 JSX、`measure()` | **浮笺视觉零变化**（hook 与隐藏的 `clip-idx` 不改变现状），长列表滚动正常 |
| 5 | `feat(theme): A 瑞士索引组件差异` | globals.css 的 A 段 | `tauri dev` 行级点验（§5）→ **停下请用户验收** |
| 6 | `feat(theme): C 雾绿工作台组件差异` | globals.css 的 C 段 | 同上 |
| 7 | *(可选)* `fix(theme): index.html 预置主题/风格，消除首帧闪变` | `index.html` 内联脚本 | 冷启动无闪变；单独回滚不影响 1–6 |

### 6.3 某组件在新风格下错位的处置路径

1. **首选：只改那一条 CSS。** 差异全部集中在 A/C 段，改 `padding/min-height/border-radius` 一处即可，不碰组件。
2. **临时隔离：删掉该组件的覆盖规则** → 它回落浮笺样式，但仍是 A/C 配色，其它组件不受影响（用于"先发整体、再修单点"）。
3. **仍不行才动 JSX**，且必须写成结构（`data-*` / hook）而不是 `style === 'a' ?` 分支；改动限制在 R1 允许的 3–5 行内。
4. **最坏情况整体回滚**：`git revert` Task 5/6（或 3–6）即可；因为无属性/`fuji` 时 `:root` 令牌 + 无差异规则 = 当前 master 行为，
   浮笺用户始终不受影响。

---

## 7. 不做什么（防范围蔓延）

- 不重构 `tailwind.config.js`、不删它、不加 `@config`；不动 `@custom-variant dark`。
- 不把风格写进 `AppSettings`/Rust；不新增 Tauri 命令/事件；不新增依赖。
- 不做"跟随系统风格"的第四种模式、不做风格预览缩略图、不做逐组件微调面板。
- 不改任何业务逻辑、`invoke` 调用点、虚拟列表定位方式（仍是 `top`，HANDOVER §5.6）、气泡动画类。
