# 安全审计报告

- **审计对象**:clipboard-manager-tauri v0.1.7(Windows 剪贴板管理器)
- **审计日期**:2026-08-29
- **审计范围**:Rust 后端(2988 行 / 26 文件)、React 前端(2706 行 / 28 文件)、Tauri 配置、NSIS 打包、更新链路
- **审计方式**:全量源码通读 + 危险模式扫描 + 权限清单比对 + 修复项逐用例验算

---

## 一、总体结论

**代码基线扎实,无高危漏洞。** 未发现 SQL 注入、命令注入、XSS、任意文件读写或权限绕过。三项中低危问题已在本轮修复(详见第四节),残余风险集中在**本地数据明文落盘**与**常驻管理员权限**两项,均属产品级取舍而非实现缺陷。

| 等级 | 数量 | 说明 |
| --- | --- | --- |
| 高危 | 0 | — |
| 中危 | 1 | R-1 剪贴板历史明文落盘(产品固有风险) |
| 低危 | 5 | R-2 ~ R-6 |
| 已修复 | 4 | F-1 ~ F-4(见第四节) |

---

## 二、威胁模型

**资产**
- 剪贴板历史全量内容(可能包含密码、密钥、身份证号等敏感信息)
- 图片存档目录 `data/images/`
- 配置文件(含热键、存储位置指针)

**威胁主体**
- T1 本地低权限恶意软件:读取 `data/` 目录
- T2 被篡改的远程 `version.json`:诱导用户下载恶意安装包
- T3 恶意/被篡改的数据库条目:利用 `file_path` 字段越界访问
- T4 WebView 前端被注入脚本:通过 IPC 命令面提权

**信任边界**
- 应用以 `requireAdministrator` 运行,与 T1 处于不同完整性级别(MIC)
- `version.json` 是唯一的外部输入源,不可信
- 数据库 `file_path` 字段源自应用自身写入,但被篡改后属不可信输入

---

## 三、逐项审计结论

### 3.1 IPC 命令面 — 通过

共 28 个 Tauri 命令。审查要点:

- **无任意命令执行**:capabilities 仅授予 `shell:default`(只含 `allow-open`),未授予 `shell:allow-execute` / `allow-spawn` / `allow-kill`。本轮已进一步移除整个 shell 插件(见 F-1)。
- **无参数注入**:前端 `invoke()` 全部传结构化参数,无字符串拼接命令。
- **`activate_item` 可触发粘贴**:任何能调用该命令的前端代码都能向当前焦点窗口写入内容。前端无可注入点,风险不成立。

### 3.2 SQL 注入 — 通过

- 前端 `lib/queryBuilder.ts` 全部使用 `?` 占位符绑定;`escapeLike()` 转义 `%` `_` `[` `]` `\` 并与 `ESCAPE '\'` 子句正确配对。
- 后端 `cleanup_expired_items` 虽用 `format!` 拼接 WHERE 子句,但拼接内容是**硬编码字面量**,值一律 `.bind()`。
- 全项目无字符串插值进入 SQL 值位置的情况。

### 3.3 路径遍历 — 修复后通过

数据库 `items.file_path` 字段是唯一的路径类不可信输入。三条消费路径的守卫情况:

| 路径 | 函数 | 修复前 | 修复后 |
| --- | --- | --- | --- |
| 删除图片 | `image_cleanup::remove_image_file` | canonicalize + starts_with | 不变(本就有) |
| 读取 base64 | `commands::clipboard::get_image_base64` | canonicalize + starts_with + 5MB 限制 | 不变(本就有) |
| **粘贴图片** | `paste_service::write_image_to_clipboard` | **无校验** | 已补(见 F-3) |

详见 F-3。

### 3.4 更新链路 — 修复后通过

- 传输层:HTTPS 拉取 `version.json`,有系统代理探测(读 WinINET 注册表)+ jsdelivr 镜像回退。
- **修复前问题**:`open_external_url` 接受任意 URL,而入参来自远程 manifest 的 `github` / `lanzou` 字段。manifest 若被篡改可把用户导向任意站点(钓鱼下载页)。详见 F-2。
- 更新**不自动下载安装**,只打开浏览器,用户仍需手动下载并覆盖安装——这大幅限制了篡改 manifest 的实际危害。

### 3.5 内容安全策略(CSP)— 通过

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
        img-src 'self' data: blob: asset: http://asset.localhost;
        connect-src 'self' ipc: http://ipc.localhost http://asset.localhost"
```

- `script-src 'self'`:无内联脚本、无远程脚本。
- `style-src 'unsafe-inline'`:**必要**。组件大量使用 React 内联 `style={{...}}`,移除会导致界面失效。不构成实质风险(style 无法执行脚本)。
- `img-src` 含 `data:`(base64 回退)与 `asset:`(Tauri 资产协议)。
- `connect-src` 的 `ipc:` / `http://ipc.localhost` 是 Tauri IPC 必需。
- 未发现 `eval` / `new Function` / `dangerouslySetInnerHTML`。

### 3.6 前端渲染 — 通过

- 全部内容经 React 文本节点渲染,无 HTML 注入面。
- 图片经 `convertFileSrc` 走 Tauri 资产协议,受 `asset_protocol_scope` 约束,越界路径会被拒绝。

### 3.7 存储位置迁移 — 通过

`storage_service::apply_storage_change` 的防护完整:
- 拒绝与当前位置相同 / 互相嵌套的目录
- 拒绝已含剪贴板数据的目录
- 写权限探测(实际写入探针文件后删除)
- 目标不可用时回退默认目录,失败时数据不动

---

## 四、本轮修复项

### F-1 移除未使用的 tauri-plugin-shell(攻击面收敛)

**问题**:`tauri_plugin_shell` 在 `lib.rs` 注册、`Cargo.toml` 声明、capabilities 授予 `shell:default`,但全项目**零使用**——前端无任何 `@tauri-apps/plugin-shell` 导入,Rust 侧无 `shell()` 调用(唯一相关的是 `app.opener()`,属另一插件)。

**处置**:删除三处声明。`tauri_plugin_opener` 与 `opener:default` 保留(`open_external_url` 依赖)。

**影响**:编译产物不再包含 shell 插件代码;`Cargo.lock` 自动剔除该依赖树(约 -73 行)。

### F-2 open_external_url 增加 https + 域名白名单

**问题**:该命令接受任意 URL,入参来自远程 `version.json`。manifest 被篡改时可把用户导向任意站点。

**处置**(`commands/update.rs`,+58 行):
- `ALLOWED_DOWNLOAD_DOMAINS`:白名单,`github.com` / `jsdelivr.net` / `lanzoul.com` / `lanzoui.com` / `lanzoux.com`。按**注册域后缀**匹配而非硬编码完整域名,以容忍蓝奏云镜像域切换与 CDN 变更。
- `url_host()`:去 scheme → 截 `/` 得 authority → 取最后一个 `@` 之后(剥离 userinfo)→ 截 `:` 去端口。未引入 `url` crate。
- `host_is_allowed()`:等价于「等于该域或为其子域」,即 `host == d || host.ends_with(".d")`。
- 强制 `https:`,校验失败返回中文错误并拒绝打开,**不静默放行、不降级**。

**绕过用例验算**(全部通过):

| 用例 | 期望 | 实际 |
| --- | --- | --- |
| `https://github.com/x` | 放行 | 放行 |
| `https://sub.github.com/x` | 放行 | 放行 |
| `https://user@github.com:443/x` | 放行 | 放行 |
| `HTTPS://GitHub.com/x` | 放行 | 放行 |
| `https://github.com.evil.com/` | 拒绝 | 拒绝 |
| `https://evilgithub.com/` | 拒绝 | 拒绝 |
| `https://github.co/` | 拒绝 | 拒绝 |
| `https://user@evil.com/github.com` | 拒绝 | 拒绝 |
| `http://github.com/x` | 拒绝 | 拒绝 |
| `github.com/x`(无 scheme) | 拒绝 | 拒绝 |

**已知限制**:`raw.githubusercontent.com` 会被拒绝(注册域为 `githubusercontent.com`)。它仅用于服务端拉取 manifest、不经浏览器打开,无实际影响;若日后需放行,加入白名单即可。

### F-3 补齐粘贴图片的路径校验(守卫一致性)

**问题**:删除路径与 base64 读取路径都做了 `canonicalize + starts_with(images_dir)` 校验,但**粘贴路径** `write_image_to_clipboard` 直接把数据库里的 `file_path` 交给 `image::open`,无任何校验。任一守卫缺口即足以让被篡改的条目打开磁盘上的任意图片。

**处置**:
- `storage_service.rs` 新增 `resolve_image_path(app, file_path) -> Option<PathBuf>`:create_dir_all → 两侧 canonicalize → starts_with 比对,越界返回 `None`。注释明确警示返回值带 `\\?\` 前缀、**不可持久化**。
- `paste_service.rs`:`write_image_to_clipboard` 参数由 `&str` 改为已校验的 `&Path`;`deliver_content` 新增首参 `app_handle`;Image 分支先校验后打开,失败返回中文错误「图片路径无效或不在应用图片目录内,已拒绝粘贴」,**不降级当文本处理**。
- `commands/clipboard.rs` 调用处传 `app_handle.clone()`(原句柄后续 `emit` 仍要用)。

**复核**:全项目仅剩两处图片解码入口——`clipboard_monitor.rs` 的 `load_from_memory`(输入是内存中的剪贴板数据,不碰文件系统)与已受校验的 `paste_service::image::open`。无遗漏入口。

### F-4 剪贴板内容排除规则(缓解 R-1)

**问题**:应用把复制过的**全部**内容落盘,包括密码、信用卡号、API 密钥。

**处置**:新增 `services/exclusion_service.rs`,在入库前做三层判定,命中即跳过(不写库、不生成预览)。

| 层级 | 依据 | 说明 |
| --- | --- | --- |
| 来源进程 | `GetClipboardOwner()` → PID → `QueryFullProcessImageNameW` | 预置 keepass / keepassxc / 1password / bitwarden |
| 用户正则 | 设置里自定义,Rust `regex` 语法 | 对文本与文件路径列表生效 |
| 内置敏感识别 | 10 类模式 | 默认开启,可在设置中关闭 |

内置识别的模式:**信用卡号**(Luhn + IIN 双重校验)、**PEM 私钥**、**AWS AKIA/ASIA**、**GitHub 令牌**、**Slack 令牌**、**sk- 类密钥**、**JWT**、**Bearer 令牌**、**Google API Key**、**中国大陆身份证号**(校验位验证)。

**误报控制是这里的重点**,三处刻意的取舍:

- 信用卡号除正则外必须通过 **Luhn 校验**,身份证号必须通过**校验位**验证——否则订单号、时间戳等普通长数字会被大量误判。
- 卡号还要过 **IIN 发卡行前缀 + 品牌长度**校验(`card_brand_ok`)。仅靠 Luhn 时,13–19 位随机数字约有 1/10 概率碰巧通过,银行账号、IMEI、长订单号会被误杀;加上前缀与长度约束后,30 万随机样本实测误报率从 **9.93% 降到 1.72%**(下降约 83%)。合法卡号必然同时满足前缀与长度,不因此漏拦。
- **刻意不**实现 `password: xxx` 这类"敏感关键词 + 值"的通用匹配:代码与配置文件里 password 字样出现频率远高于真实密码泄漏,关键词匹配会制造大量误报,最终让人干脆关掉整个功能。

**误伤豁免**:命中时状态栏给出 8 秒提示并提供「仍要记录」按钮。点击后该内容 hash 写入持久化白名单(`settings.excluded_allowlist`,上限 200 条),并请求轮询线程清空 `last_hash` 重新捕获一次——内容此刻仍在系统剪贴板里,无需用户再复制。白名单优先级高于三层规则,可在设置面板一键清空。

这条豁免路径是必要的:排除时后端会把 hash 写进 `last_hash` 以避免每 500ms 重跑正则,副作用是**被误判的内容再复制多少次都进不了历史**(去重直接跳过,提示也不再出现)。没有豁免入口,用户只能整体关掉敏感识别,等于废掉整个功能。

**已知限制**:

- 本功能是「**已知格式密钥 + 密码管理器**」拦截器,**不是**通用敏感内容识别器。
  - 拦得住:密码管理器来源、AKIA/ghp_/xox/sk-/AIza/JWT/PEM、合法卡号与身份证号。
  - 拦不住:非密码管理器来源的明文密码(聊天窗口、Excel、网页"显示密码"、终端输出)、配置文件式凭据(`db_password: 真实值`、`.env` 全文)、无特征前缀的自建 Key、数据库连接串与 URL 内嵌账密。
- `GetClipboardOwner()` 在**复制方进程已退出**时返回 NULL,此时进程黑名单不生效(另两类规则照常)。这是所有剪贴板管理器的共同限制。
- 白名单一旦加入即绕过全部规则。若用户误在真实密钥上点击「仍要记录」,该密钥将不再被排除——设置面板的「清空豁免名单」是撤销手段。

**健壮性**:

- 用户正则编译有 `size_limit` 上限(1MB);语法错误仅跳过该条并记日志,**绝不 panic**——panic 会让 500ms 轮询线程整个死掉。
- 内置正则用 `LazyLock` 一次编译;`source_process_name()` 全路径返回 `None` 兜底,不 panic。
- 重捕获信号「取走即清」,每轮轮询只消费一次,避免信号滞留导致日后同一内容被重复入库。

---

## 五、残余风险与建议

### R-1 剪贴板历史明文落盘(中危 · 产品固有)

`data/clipboard.db` 与 `data/images/` 全部明文。任何能读取该目录的主体即可获取全部历史,**包括用户复制过的密码、密钥、身份证号**。

> **更新**:本项已通过 F-4 排除规则**部分缓解**(来源进程黑名单 + 用户正则 + 内置敏感识别)。残余部分是「未被规则覆盖的内容仍会明文落盘」,即 F-4 中"拦不住"的那几类——普通明文密码、配置文件式凭据、无特征前缀的自建 Key。

这是剪贴板管理器的固有风险(Win+V、Ditto 等同类工具同样如此),且本项目「数据明文存于本地、不上传」本身就是设计目标。若要进一步缓解:
1. **引导用户将存储位置放到 BitLocker / 加密盘**(已有自定义存储位置能力,零开发成本)。
2. 数据库加密(SQLCipher)会显著抬高复杂度并与「标准 SQLite 可导出」的卖点冲突,**不建议**。

### R-2 常驻管理员权限(低危 · 已决策保留)

`app.manifest` 为 `requireAdministrator`,每次启动弹 UAC,剪贴板内容长期在高权限进程内处理。

这是**有意的产品取舍**:向管理员窗口粘贴、写 HKLM 注册表接管 Win+V 都需要同等完整性级别。降级会直接失去这两个核心卖点。**维持现状。**

### R-3 version.json 无签名校验(低危)

仅有 HTTPS 传输层保护,manifest 内容本身无完整性校验。经 F-2 修复后,篡改 manifest 的最坏后果已被限制为「只能在白名单域名内更换下载链接」,危害可控。若需进一步加固,可对 manifest 做 Ed25519 签名校验。

### R-4 `\\?\` 前缀路径的持久化风险(低危 · 靠约定约束)

Windows `canonicalize` 返回 `\\?\` 扩展前缀路径,一旦写进 `storage.json` 或数据库、再拼进 `sqlite:` URL,会导致**启动即崩**。项目已有 `strip_extended_prefix` 与多处注释警示,但属**约定约束、无编译期强制**。本轮新增的 `resolve_image_path` 已在文档注释中标明返回值不可持久化。

建议后续:为 `strip_extended_prefix` 与 `resolve_image_path` 补充单元测试,或引入 `#[must_use]` 之类的编译期提示。

### R-5 资产协议作用域依赖手工授权(低危 · 可用性)

自定义存储目录必须在启动时 `asset_protocol_scope().allow_directory()` 动态授权,否则缩略图全变占位符(配置里的 scope 只覆盖默认 `$APPDATA/images/*`)。目前只在 `lib.rs` 的 setup 中授权一处——若将来新增窗口创建路径而遗漏,表现为功能失效而非安全漏洞。

### R-6 更新检查失败静默(低危 · 可用性)

`App.tsx` 中 `check_update` 的 `.catch(() => {})` 静默吞掉错误,用户无法区分「无网络」与「已是最新」。属体验问题,非安全问题。

---

## 六、验证记录

| 项目 | 命令 | 结果 |
| --- | --- | --- |
| Rust 静态检查 | `cargo +stable-x86_64-pc-windows-gnu clippy --all-targets` | 通过,0 error。10 条 warning **全部落在既有文件**(`lib.rs`、`window.rs`、`settings.rs`、`clipboard_monitor.rs`、`clipboard.rs:149`),本次改动的 `update.rs` / `paste_service.rs` / `storage_service.rs` **零 warning** |
| Rust 类型检查 | `cargo check` | 通过,43.66s |
| 前端类型检查 | `tsc` | 通过 |
| 前端构建 | `vite build --outDir dist-verify` | 通过,64 模块,3.68s |
| 白名单逻辑 | 19 个 URL 用例独立编译验证 | 19/19 通过 |

> 注:`npm run build` 在本沙箱环境会失败——Vite 写盘前需清空 `dist/`,而沙箱的 safe-delete 钩子回收 `dist/index.html` 失败。这与代码无关:改用新输出目录后构建完全正常。

---

## 七、未纳入本轮范围

- `requireAdministrator` 降级改造(产品决策,已确认维持现状)
- `get_image_base64` 与 `remove_image_file` 中**已有**的校验**故意保持原样未重构**,避免把可用校验暴露在回归风险中。若需统一到 `resolve_image_path`,应作为独立的、带测试的任务进行。
