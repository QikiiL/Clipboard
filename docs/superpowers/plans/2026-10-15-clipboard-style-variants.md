# 剪贴板三套风格变体原型 · 实施计划

> **给执行代理：** REQUIRED SUB-SKILL：`superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`；步骤用 `- [ ]` 跟踪。**每个任务派一个独立子代理**；**阶段 1（Task 2–3）结束必须停下请用户验收**，未获确认不得开始 Task 4。

**Goal:** 在 `docs/superpowers/previews/clipboard-style-variants/index.html` 产出一个 `file://` 可打开、零依赖、零网络的三列并排风格变体原型，让用户从 A 瑞士索引 / B 石墨控制台 / C 雾绿工作台 中选定一个方向。

**Architecture:** 单 HTML 文件（HTML + CSS + JS 全内联），靠 `[data-variant]` 作用域 CSS + 每列独立 `state` 对象 + 事件委托到列根实现三套隔离；**全文件禁用 `id`**。交付分两阶段：**阶段 1** 先给用户一个"能打开的骨架"——三列 + 真实示例内容 + 每列的设计假设标注（中性样式、无交互），拿到用户对方向与假设的认可；**阶段 2** 再填完整变体样式、浅深两态与核心点击。原型是**设计探索稿**，不复刻生产行为。

**Tech Stack:** 原生 HTML/CSS/JS（无框架、无构建、无 CDN）、CSS `oklch` 变量、`@container` 容器查询、内联 SVG（path 数据照抄 `src/components/icons.tsx`）。

**权威来源（本文不重复）：** 规格 `docs/superpowers/specs/2026-10-15-clipboard-style-variants-design.md`——令牌值照抄 §4.1、示例内容照抄 §5、点击矩阵照抄 §6.3、验收照抄 §10；本文只补"怎么做、怎么验"。

**硬边界：** 不改 `src/` `src-tauri/` `index.html` `package.json` `globals.css`；不加依赖/构建；不读写真实剪贴板、不调 Tauri API；不做 logo/字标；不删 `D:\copy\ui-redesign\` 旧预览；**不把 6 场景 × 3 做成生产仿真**——不做真实拖拽框选、不做连续粘贴队列、不做分组增删改、不做逐帧动画、不做 20+ 场景；S3/S5/S6 只做静态替换。

**产物与验证策略：** 唯一产物是上面那个 `index.html`（§10.1 要求该目录恰好 1 个文件）——**不新增任何测试文件**，也不建 `plans/verification/`；**不建 worktree**（用户已选择直接在当前 master 上做）。静态约束用临时 Node one-liner（跑完即弃）；浏览器验证用**本机 Edge 155 + Node 24 原生 `WebSocket` 直连 CDP**（环境已确认可用）：截图、点击、抓 Console/异常都能做，**不安装 Playwright 或任何依赖**；临时脚本一律放 `%TEMP%`、不提交；§10 人工清单仍是最终依据。

```powershell
# ① 截图（headless，免脚本）；--blink-settings=scriptEnabled=false 用于 no-JS 验收
& "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --headless=new --disable-gpu --window-size=1440,900 --screenshot="$env:TEMP\clip-var-p1-1440.png" "file:///D:/copy/clipboard-manager-tauri/docs/superpowers/previews/clipboard-style-variants/index.html"
# ② 点击 / Console / 异常：先起 CDP，再跑临时脚本（Node 24 自带 WebSocket，无依赖）
& msedge --headless=new --remote-debugging-port=9222 --user-data-dir="$env:TEMP\clip-cdp-profile" "file:///D:/copy/clipboard-manager-tauri/docs/superpowers/previews/clipboard-style-variants/index.html"
node "$env:TEMP\clip-cdp.mjs"   # GET /json/list 取 webSocketDebuggerUrl → Runtime.enable + Log.enable → Runtime.evaluate 点击 → Page.captureScreenshot → 打印 console/异常
```

内部顺序固定：① head+style ② 外壳令牌 ③ 6 个变体令牌块 ④ 组件层 ⑤ 断点 ⑥ 静态骨架 ⑦ `createPanel()` 实例化 3 次。

---

### Task 1: 先让验证失败（红）

**Files:** 无改动（只读验证）。

- [ ] **Step 1: 跑 Gate 1（静态约束），确认现在失败**

```powershell
node -e "const fs=require('fs'),d='docs/superpowers/previews/clipboard-style-variants',f=d+'/index.html';if(!fs.existsSync(f)){console.error('FAIL: missing '+f);process.exit(1)}const s=fs.readFileSync(f,'utf8'),bad=[[/navigator\.clipboard/,'navigator.clipboard'],[/execCommand/,'execCommand'],[/invoke\(/,'invoke('],[/__TAURI__/,'__TAURI__'],[/fetch\(/,'fetch('],[/XMLHttpRequest/,'XMLHttpRequest'],[/localStorage/,'localStorage'],[/sessionStorage/,'sessionStorage'],[/[\s<]id\s*=/,'id='],[/(?:src|href)=[\x22']https?:/,'external url'],[/@import\s+url\(\s*[\x22']?https?:/,'@import http']].filter(p=>p[0].test(s)).map(p=>p[1]);const files=fs.readdirSync(d);if(bad.length||files.length!==1){console.error('FAIL hits='+JSON.stringify(bad)+' files='+JSON.stringify(files));process.exit(1)}console.log('PASS: 1 file, 0 forbidden hits')"
```

Expected（本机实测已确认）：`FAIL: missing docs/superpowers/previews/clipboard-style-variants/index.html`，exit 1。

- [ ] **Step 2: 跑 Gate 2（结构，stage=2），确认现在失败**

```powershell
node -e "const fs=require('fs'),f='docs/superpowers/previews/clipboard-style-variants/index.html';if(!fs.existsSync(f)){console.error('FAIL: missing '+f);process.exit(1)}const s=fs.readFileSync(f,'utf8'),stage=process.argv[1]||'1';const g1=['class=\x22no-js\x22','data-variant=\x22a\x22','data-variant=\x22b\x22','data-variant=\x22c\x22','data-scene=\x22s1\x22','data-role=\x22assumptions\x22','data-role=\x22panel\x22','&lt;script&gt;alert'],g2=['data-theme=','container-type','tabular-nums','data-requires-js','清空搜索','暂无剪贴板记录','v1.3.0'];const miss=g1.concat(stage==='2'?g2:[]).filter(n=>!s.includes(n));if(miss.length){console.error('MISSING '+JSON.stringify(miss));process.exit(1)}console.log('PASS structure stage='+stage)" 2
```

Expected：`FAIL: missing ...`，exit 1。

- [ ] **Step 3: 记录基线**：`git status --short` → 仅本计划文件已提交后的干净状态。红态是证据不是产物，**不提交任何东西**。

Gate 2 的断言只覆盖**静态骨架里必然存在**的标记：默认场景是 S1（**不要**断言动态场景值如 `s6`，会假失败）；`data-role` 是静态骨架与 JS 渲染共用的唯一选择器契约（**统一用 `data-role`，不用 class 选择器驱动 JS**）。

---

### Task 2（阶段 1）: 可打开的三列骨架 + 真实内容 + 假设标注

**Files:** Create `docs/superpowers/previews/clipboard-style-variants/index.html`

- [ ] **Step 1: 写外壳与静态三列（中性样式，不做变体差异）**

```html
<!doctype html>
<html class="no-js" lang="zh-CN">
<head><meta charset="utf-8"><title>剪贴板 · 三套风格变体</title>
<style>
  :root{--font:'Segoe UI Variable Text','Segoe UI','Microsoft YaHei UI',system-ui}
  body{margin:0;font-family:var(--font);background:#f4f4f2;color:#1a1a1a}
  .board{display:flex;gap:24px;padding:24px;align-items:flex-start;flex-wrap:wrap}
  .col{flex:1 1 380px;min-width:320px;max-width:600px}
  .colhead{display:flex;gap:8px;align-items:baseline;padding:4px 2px 8px;font-size:13px}
  .assumptions{border:1px dashed #b9b9b4;border-radius:4px;padding:8px 10px;font-size:12px;line-height:1.6;margin-bottom:8px}
  .panel{width:420px;max-width:100%;height:520px;background:#fff;border:1px solid #dcdcd8;display:flex;flex-direction:column;overflow:hidden}
  .titlebar,.searchrow,.statusbar{display:flex;align-items:center;gap:8px;padding:0 10px;border-bottom:1px solid #e6e6e2}
  .titlebar{height:46px}.searchrow{height:36px}.statusbar{height:36px;border-top:1px solid #e6e6e2;border-bottom:0;font-size:11px}
  .tabs{display:flex;gap:4px;padding:6px 10px;overflow-x:auto}
  .list{flex:1;overflow-y:auto;padding:4px 8px}
  .row{display:flex;align-items:center;gap:8px;min-height:52px;padding:6px}
  button{font:inherit;background:none;border:0;cursor:pointer}
  :focus-visible{outline:2px solid currentColor;outline-offset:2px}
</style></head>
<body>
<div class="board" data-board>
  <section class="col" data-variant="a">
    <div class="colhead"><strong>A · 瑞士索引</strong><span>浅灰白 / 清晰分隔 / 朱红只做定位</span></div>
    <ul class="assumptions" data-role="assumptions"><!-- Step 3 --></ul>
    <div class="panel" data-role="panel" data-scene="s1"><!-- Step 2 --></div>
  </section>
  <!-- 列 b / 列 c：由列 a 复制，仅改 data-variant 与列头文字（内容三套共享） -->
</div>
<noscript><p style="padding:12px">本预览的交互需要 JavaScript；当前为静态展示</p></noscript>
<script>document.documentElement.className='js';</script>
</body></html>
```

- [ ] **Step 2: 在每列 `data-role="panel"` 内写静态面板 DOM（顺序不可变，§6.1），部件一律用 `data-role`**

```
标题栏 46px  data-role="titlebar"：纯文本「剪贴板」+ [置顶][保持打开][连贴][设置] + [—][□][×]
搜索行 36px  data-role="search"：[🔍 搜索剪贴板…]
分组页签     data-role="tabs"：( 全部 ( 收藏 ( 示例分组 A ( 示例分组 B (＋)
列表         data-role="list"：10 行 data-role="row"（连贴横幅 data-role="banner" 仅 S5 渲染）
状态栏 36px  data-role="statusbar"：10 条 · 2 收藏 · 存储 · v1.3.0  [单击粘贴] ● 监听中
```

行模板（10 行只改图标/文本/元信息/星标，值逐字取自 §5 表）：

```html
<div class="row" data-role="row" data-n="01" data-type="text">
  <span data-role="idx">01</span>
  <span data-role="ico"><svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><path d="…"/></svg></span>
  <div data-role="body"><p data-role="title">npm run tauri build</p></div>
  <div data-role="meta">
    <button data-role="star" aria-label="收藏">…</button><span data-role="time">刚刚</span>
    <div data-role="acts"><button data-role="preview" aria-label="预览">…</button><button data-role="fav" aria-label="收藏">…</button><button data-role="group" aria-label="分组">…</button><button data-role="del" aria-label="删除">…</button></div>
  </div>
</div>
```

图标 path 数据**逐字复制** `src/components/icons.tsx`（`SearchIcon XIcon PlusIcon PinIcon SettingsIcon TextIcon LinkIcon ImageIcon FileIcon StarIcon StarOutlineIcon FolderIcon FolderFillIcon TrashIcon EyeIcon LayersIcon AppWindowIcon MinusIcon MaximizeIcon ClipboardIcon`），不重绘、不改 viewBox。

- [ ] **Step 3: 写每列假设标注**（`data-role="assumptions"`，阶段 1 的评审物，各 3 条 + "怎么看"）

- **A**：A1 贯穿全宽分隔线比卡片更利于快速扫描（看：不读列头能否一眼看出是"索引"）；A2 行左两位序号栏支持按位置记忆（看：能否凭序号回到第 07 条）；A3 单一朱红只做定位不参与装饰（看：红色是否只在页签/序号/星标/焦点环）。
- **B**：B1 40px 紧凑行一屏 ≥9 条时效率更高（看：同样高度是否装下更多）；B2 等宽数字让序号与时间可对齐扫读（看：数字是否成一条线）；B3 荧绿只表状态、不用于选中（看：绿色是否只出现在监听/已粘贴/焦点）。
- **C**：C1 无描边、靠明度分层更柔和但不丢层级（看：能否找到"卡片边框"）；C2 60px 行高与 1.7 行距更易读但一屏更少（看：读数与容量取舍）；C3 低彩度雾绿做强调仍保持专注（看：强调色是否抢走注意力）。

- [ ] **Step 4: 第 08 条转义哨兵**：静态 HTML 写成 `&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt; &amp; 引号 &quot; ' 的转义用例`；JS 侧一律经 `esc()` 输出，**任何路径都不得直接拼接 `innerHTML`**。

- [ ] **Step 5: 跑 Gate 1 + Gate 2(stage=1) 转绿** → `PASS: 1 file, 0 forbidden hits` / `PASS structure stage=1`。

- [ ] **Step 6: 浏览器验证**：用上面 ① 命令截 1440 / 1000 两张（`--window-size=1440,900` / `1000,900`，输出 `$env:TEMP\clip-var-p1-1440.png` / `clip-var-p1-1000.png`），用 `read_image` 检查三列同屏、无横向滚动、无错位、10 行可读、假设块可见；用 ② 脚本抓 Console（0 错误 0 警告）；需人眼交互时再 `Start-Process -FilePath "D:\copy\clipboard-manager-tauri\docs\superpowers\previews\clipboard-style-variants\index.html"`（PowerShell 下不要用 `start ""`）。

---

### Task 3（阶段 1 收尾）: 提交 + **用户验收检查点**

**Files:** 无新增。

- [ ] **Step 1:** `git status --short` → 仅 `?? docs/superpowers/previews/`。

- [ ] **Step 2: 提交阶段 1** → `git add docs/superpowers/previews/clipboard-style-variants/index.html && git commit -m "feat(preview): stage 1 three-column skeleton with design assumptions"`

- [ ] **Step 3: 停下请用户验收（硬门）**：发两张截图 + 每列 3 条假设，用 `ask_user_question` 让用户选 ① 认可，进入阶段 2 / ② 调整方向 / ③ 调整假设。**未获 ① 不得开始 Task 4**；选 ②/③ 则改 Task 2 产物 → 重跑 Step 5–6 → 追加提交 → 再问一次。

---

### Task 4（阶段 2）: 变体令牌与组件层（A/B/C × 浅/深）

**Files:** Modify 同文件。

- [ ] **Step 1: 插入 6 个变体令牌块**，值逐字照抄 §4.1（不重算、不微调）：

```css
[data-variant="a"][data-theme="light"]{--bg:oklch(96.2% 0.003 95);--surface:oklch(99.2% 0.002 95);--surface-2:oklch(94% 0.004 95);--text:oklch(19% 0.005 260);--muted:oklch(45% 0.008 260);--faint:oklch(62% 0.008 260);--hair:oklch(85% 0.004 95);--accent:oklch(55% 0.19 30);--accent-deep:oklch(48% 0.19 30);--accent-soft:oklch(94% 0.03 30);--star:oklch(62% 0.16 30);--danger:oklch(52% 0.2 27)}
```

其余 5 块同构（`b/c` × `light/dark`）；B 另含 `--select` / `--select-rule`；C 的 `--hair` 带 `/ 0.7`。每列根挂 `data-theme`，切换只影响本列。

- [ ] **Step 2: 写组件层样式**（全部作用域到 `[data-variant="x"]`，落实 §4.2）：

- **A**：圆角 2px；序号栏 34px 等宽两位；行间 1px `--hair` **贯穿全宽**、行间距 0；选中＝序号变 `--accent` + 底 `--surface-2`；页签直角分段控件（选中＝底纹 + 文字朱红 + 2px 下划线）；无阴影、无 pill、**无侧边强调条**。
- **B**：行高 40px；行首 6px 状态圆点（默认中性暗点 / 选中 `--select-rule` / 已粘贴 `--accent` / 暂停琥珀）；全量 `font-variant-numeric:tabular-nums`；分区头与状态栏 10px + `letter-spacing:.16em`；页签紧凑 pill（选中＝中性填充，不用绿）；**无发光、无扫描线、无网格背景、无渐变**。
- **C**：行无描边、无侧边竖条、圆角 14px、行间 12px 间隙；靠 `--bg`/`--surface` 明度差分区；选中＝仅升到 `--surface-2`；页签低对比 pill（无下划线无描边）；空状态上下留白 64px、行距 1.8。

- [ ] **Step 3: 断点（容器查询 + 比较器堆叠）**

```css
.panel{container-type:inline-size}
@container (max-width:360px){ .statusbar .storage,.statusbar .ver{display:none} .list{padding:2px 6px} .tabs{overflow-x:auto} }
@container (min-width:520px){ .row{min-height:60px} .list{padding:4px 16px} .title{font-size:14px} }
@media (max-width:1100px){ .board{flex-direction:column} }
```

1440px 下 3×420 + 2×24(gap) + 2×24(padding) = 1356 ≤ 1440，三列并排成立；**<1100px 纵向堆叠**（§6.3）。

- [ ] **Step 4: 验证 + 提交**：Gate 1 + Gate 2(stage=2) 双 PASS；用 ① 命令截 6 张 `$env:TEMP\clip-var-{a,b,c}-{light,dark}.png`（先用 ② 脚本 `Runtime.evaluate` 切本列浅深再 `Page.captureScreenshot`），`read_image` 逐条核对 §4.3 锚点 A1–A6 / B1–B6 / C1–C6；然后 `git add docs/superpowers/previews/clipboard-style-variants/index.html && git commit -m "feat(preview): variant tokens, component layer, container breakpoints"`

---

### Task 5（阶段 2）: 交互内核（列头控件 + 核心点击，最小实现）

**Files:** Modify 同文件。

- [ ] **Step 1: `createPanel(root, variantId)` 工厂 + 事件委托**：`state = { scene:'s1', theme:'light', width:420, items:[…], selected:[], log:[], notice:null, error:null, band:null }`；每列一实例，`createPanel(colRoot,'a'|'b'|'c')` 调用 3 次；事件在列根委托、`closest('[data-role]')` 分派；**禁用 `id`**。

- [ ] **Step 2: 列头控件 + 便利按钮**：列头（`data-requires-js`）＝本列浅深 / 本列宽度 320/420/520/600 / 本列场景 s1–s6；顶部三个便利按钮＝全部切浅色 / 全部切深色 / 全部回 S1。

- [ ] **Step 3: 按 §6.3 实现最小点击集**：条目单击（日志追加 `粘贴 · #01`、该行高亮 600ms、**不写系统剪贴板**）；悬停展开 4 个操作按钮（行高不跳动）；☆ 切换收藏 + 状态栏计数同步；垃圾桶**行内**二次确认（「确认删除？」+「取消」）；文件夹图标 3 项下拉（未分组 + 2 个示例分组），选后行内出现分组图标；页签过滤；搜索 300ms 防抖；监听圆点「监听中/已暂停」互切；齿轮打开设置模态（遮罩 / × / Escape 关闭）；设置「深色模式」只切本列。

- [ ] **Step 4: Escape 分层**（禁止全局单点处理）：行内删除确认 → 设置模态 → 模态内确认 → 搜索框清空，每层只消费自己那层。

- [ ] **Step 5: 验证 + 提交**：Gate 1；用 ② 脚本逐条 `Runtime.evaluate` 触发 §6.3 最小集（点击后读 `document` 断言日志行/高亮/计数，并打印 Console 与异常，须为空）；**用剪贴板查看器确认单击条目后剪贴板内容不变**；截图 `$env:TEMP\clip-var-click-{hover,del,modal,band}.png` 并 `read_image` 检查行高不跳动、确认态可见；然后 `git add docs/superpowers/previews/clipboard-style-variants/index.html && git commit -m "feat(preview): per-column interaction core"`

---

### Task 6（阶段 2）: 六场景最小内容与边界

**Files:** Modify 同文件。

- [ ] **Step 1: S2 空态 4 文案**（照抄 `src/components/ClipboardList.tsx:100-147`，不改标点、不新增文案）：`没有匹配「{query}」的记录` / `换个关键词试试` / 按钮「清空搜索」；`还没有收藏` / `点条目上的 ☆ 把常用内容收进来`；`这个分组还是空的` / `点条目上的文件夹图标把内容归进来`；`暂无剪贴板记录` / `复制内容后将自动出现在这里`。结构固定「图标 → 标题 → 描述 → 可选按钮」；「清空搜索」回 S1、计数恢复 10。

- [ ] **Step 2: S3/S5/S6 只做静态替换**：S3 居中一行「加载中…」、**不加定时器**；S5 第 01/04 条选中 + 序号徽标 1/2、横幅为框选阶段文案，「开始」只把文案换成 `剩余 2/2`、「完成」回 S1 并清空选中、**不做拖拽框选**；S6 列表上方一行 `粘贴失败：目标窗口未响应`（`--danger`、11px）+ 一条中性居中提示条「示例提示：这是一条模拟提示」，静态、可复现、**无随机故障注入**。

- [ ] **Step 3: S4 设置模态**：遮罩 `rgba(0,0,0,.5)`，卡片 `max-width:448px`、`max-height:90vh`，头部「设置 + v1.3.0 + ×」，主体 6 分区（外观/通用/快捷键/隐私/数据/关于）纵向滚动、无底部按钮；开关只切预览内状态，不落盘、不发请求。

- [ ] **Step 4: 窄屏与 no-JS**：320px 隐藏状态栏存储大小与版本号、列表 padding 收窄、页签横向滚动（滚动条 4px）、横幅按钮换行、模态 `margin:0 12px`，无横向滚动无溢出；520/600px 行高 +8px、字号 +0.5~1px、列表 padding 16px；`.no-js [data-requires-js]{display:none}`，每列显示完整默认场景 + `<noscript>` 提示，不得出现空白列或错位。

- [ ] **Step 5: 验证 + 提交**：Gate 1 + Gate 2 stage=2 双 PASS；截图 `$env:TEMP\clip-var-320.png`（列宽 320）、`clip-var-nojs.png`（① 命令加 `--blink-settings=scriptEnabled=false`），`read_image` 检查 320 无溢出、no-JS 三列完整；然后 `git add docs/superpowers/previews/clipboard-style-variants/index.html && git commit -m "feat(preview): scenes, narrow breakpoints, no-js fallback"`

---

### Task 7: 全量验证与收尾

**Files:** 无新增（只修遗漏）。

- [ ] **Step 1: 命令级验收（§10.1–§10.5）**：Gate 1 + Gate 2 stage=2 双 PASS；`git status --short` 只显示 `docs/superpowers/previews/` 新增；`git diff --stat -- src src-tauri package.json index.html` 输出为空。

- [ ] **Step 2: 人工验收（§10.6–§10.17）逐条走查并记录证据**：Network 除 document 外 0 请求、Console 0 错误 0 警告；1440 三列并排 / 1000 纵向堆叠均无横向滚动；6 场景 × 3 套可切且切走再回不残留；每套浅深核对 A1–A6 / B1–B6 / C1–C6；对比度（正文 4.5:1、图形 3:1）；Tab 可达 + 焦点环可见 + Escape 分层逐层验证；单击条目后剪贴板不变；第 08 条渲染为可见文本且无 `alert`；禁 JS 刷新三列完整；并排一眼可区分且浅深仍认出同一套；界面标识为纯文本「剪贴板」、无 logo/字标。

- [ ] **Step 3: 收尾**：若有遗漏修正则 `git add docs/superpowers/previews/clipboard-style-variants/index.html && git commit -m "fix(preview): address final verification findings"`；然后向用户汇报文件路径 + 三列并排截图 + §10 十七条验收结论表（通过/不通过 + 证据），请用户在三套中选定方向。

---

## 自检

- **规格覆盖**：§4 → T4；§5 → T2；§6.1 → T2；§6.2 → T6；§6.3 → T5；§7 → T4/T5；§8 → T2/T4；§9 → T6；§10 → T1/T3/T7；§11 → T2/T6。无缺口。
- **占位符与一致性**：0 处 TBD/TODO，命令均可直接粘贴执行（Gate 1/2 已实测：红态 exit 1、stage 参数解析正常）；全篇统一 `data-variant` / `data-theme` / `data-width` / `data-scene` / `data-role` / `data-requires-js`、`createPanel(root, variantId)`、`state.{scene,theme,width,items,selected,log,notice,error,band}`、`esc()`/`el()`/`svg()`；Gate 2 只断言静态骨架必然存在的标记（默认场景 S1），动态场景值不参与断言。
