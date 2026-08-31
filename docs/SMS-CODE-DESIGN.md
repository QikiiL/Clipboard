# 短信验证码自动捕获 · 技术方案

> 状态:设计稿,待拍板后实施
> 调研对象:TeleLink (github.com/nicholasxdavis/telelink, Python, MIT)
> 目标:手机收到含验证码的短信 → 自动提取验证码 → 写入 Windows 剪贴板 → 应用内提示

---

## 1. 结论(TL;DR)

**采用 TeleLink 的核心路径:Windows `UserNotificationListener` API 轮询通知中心,过滤出 Phone Link 的通知,提取验证码后写入剪贴板。**

- 安卓和 iPhone **同一个方案**,因为两条路最终都汇到「Phone Link 弹 Windows 通知 toast」
- 不需要 ANCS 蓝牙协议、不需要 UIA 抓窗口、不需要手机装任何 App
- 数据全程本地,零云端
- 唯一新增依赖:`windows` crate 的几个 feature(WinRT API 无法用裸 `#[link]` 声明绕过,见 §7.8)

---

## 2. TeleLink 是怎么实现的(源码调研结论)

调研了 TeleLink 的完整源码树(190 个文件)和关键模块源码。**重要发现:TeleLink 没有用 ANCS,也没有直接抓 Phone Link 窗口。**

### 2.1 核心捕获路径:`notifications.py` → `UserNotificationListener`

```
手机收到短信/iMessage
  → Link to Windows(手机侧系统级转发)
  → Phone Link(PC)弹 Windows toast 通知
  → Windows 通知中心
  → UserNotificationListener.GetNotificationsAsync(Toast) 轮询
  → 按 App 身份过滤出 Phone Link 的通知
  → 从 toast 的 text elements 提取 标题(发送者) + 正文(短信内容)
  → 去重后写入 caught.jsonl
```

关键源码逻辑(摘自 `telelink/notifications.py`):

```python
# 1. 拿到系统通知监听器(WinRT 官方 API)
from winrt.windows.ui.notifications.management import UserNotificationListener
listener = UserNotificationListener.current

# 2. 请求权限(用户需在 设置>隐私>通知 里允许)
status = await listener.request_access_async()
#   == UserNotificationListenerAccessStatus.ALLOWED 才能继续

# 3. 轮询通知中心的所有 toast(默认 2 秒一次,空闲时退避到 4-8 秒)
notifications = await listener.get_notifications_async(1)  # 1 = Toast

# 4. 按 App 身份过滤 Phone Link
def matches_phone_link(user_notification, config):
    aumid  = user_notification.app_info.app_user_model_id.lower()
    display = user_notification.app_info.display_info.display_name.lower()
    if "yourphone" in aumid or "phone link" in display:
        return True
    ...

# 5. 提取文本:遍历 toast 绑定的 text elements
#    texts[0] = 标题(发送者名字),其余 = 正文(短信内容)
#    拿不到时降级解析 notification.content.get_xml() 里的 <text> 标签

# 6. 去重:按 (app_id, notification_id) 记录已见集合,
#    每轮用「当前通知中心还存在的 id 集合」修剪已见集合(防 id 复用误杀)
```

### 2.2 TeleLink 中其他路径的角色(我们不需要)

| TeleLink 模块 | 用途 | 对本项目 |
| --- | --- | --- |
| `watcher.py` + watchdog | 监听 Phone Link 的照片文件夹变化(收图片) | 不需要,我们只要文本 |
| `phone_link_sqlite.py` | 读 Phone Link 本地 SQLite,但**只用来导入联系人**,不读短信 | 不需要 |
| `ui_automation/` (pywinauto) | 自动点 Phone Link 的「发送」按钮(发短信用) | 不需要,我们只收不发 |
| `messaging_adb.py` | 安卓 USB/ADB 收发(需 USB 连接) | 不需要 |
| `ocr.py` / `analyze_snapshot.py` | 截图 OCR,给 AI 看附件 | 不需要 |

**结论:收短信验证码只需要复刻 `notifications.py` 这一条路径,约 200 行核心逻辑,Rust 重写完全可行。**

### 2.3 值得照抄的三个设计细节

1. **去重键 = (AUMID, notification_id)**,且每轮用「当前活跃通知集合」修剪已见集合——通知从通知中心清除后,同 id 的新通知不会被误判为已见
2. **双重身份匹配**:AUMID 含 `yourphone`(语言无关,最可靠)+ 显示名含 `phone link`。我们再加中文显示名「手机连接」
3. **提不到 text elements 时降级解析 toast XML** 的 `<text>` 标签

---

## 3. 备选方案对比

| 方案 | 原理 | 优点 | 缺点 | 结论 |
| --- | --- | --- | --- | --- |
| **A. UserNotificationListener**(TeleLink 同款) | 轮询 Windows 通知中心 | 官方 API、安卓/iPhone 通用、无需手机 App、无需蓝牙配对细节 | 需用户开一次隐私权限;依赖 Phone Link 连接存活 | ✅ **采用** |
| B. ANCS/MAP 直连蓝牙(adit 项目的方式) | 绕过 Phone Link,自己实现 BLE 蓝牙协议栈 | 不依赖 Phone Link;iPhone 可读全量短信 | 仅 iPhone(安卓无 ANCS);要自建配对;Phone Link 已配对时会冲突;工作量 5-10 倍 | ❌ 过度设计 |
| C. UIA 抓 Phone Link 窗口 | pywinauto/UIA 读窗口控件 | 直观 | 要求 Phone Link 窗口常开、控件树随版本变、中英文名不一,极脆 | ❌ TeleLink 也只用来「点发送」,不用于读 |
| D. 读 Phone Link SQLite | 直接查本地库 | 可拿全量历史 | 表结构无文档且随版本变;运行中被锁(需复制后读);**iPhone 的消息根本不落本地库**(iOS 只转发通知) | ❌ 不可行(iPhone)+ 不稳定 |

方案 A 对安卓和 iPhone 是**同一条代码路径**,差异只在手机侧前置设置。

---

## 4. 集成架构(本项目)

### 4.1 数据流

```
[手机] 短信到达 → Link to Windows 转发
[PC]   Phone Link 弹 toast → 通知中心
[Rust] sms_code_service 轮询(2s)
         ├─ 过滤:AUMID 以 microsoft.yourphone 开头 / 显示名 phone link|手机连接
         ├─ 提取:标题(发送者) + 正文
         ├─ extract_code():关键词 + 4-8 位数字 → 验证码
         ├─ 去重:(aumid, notification_id) 已见集合
         └─ 命中 →
              ├─ 写剪贴板(arboard set_text,不加 suppress)
              │    → 现有 clipboard_monitor 500ms 内自动把它记进历史 ✅
              │      (零额外代码,验证码天然出现在历史列表里)
              └─ emit("sms-code-copied", { code, sender })
                   → 前端居中 toast「验证码 123456 已复制」
                   → 复用 v0.1.9 的 tauri-plugin-notification(失焦时系统通知)
```

**关键衔接点:写剪贴板不加 `suppress`**。现有监控器会把验证码当作普通剪贴板内容自动入库,历史里直接能翻到,这是最优雅的集成方式。

### 4.2 模块划分

```
src-tauri/src/services/sms_code_service.rs   # 新增,核心服务
  ├─ ensure_access()        # 权限检查/请求(线程池阻塞调用)
  ├─ poll_once()            # 一轮轮询:过滤→提取→去重
  ├─ extract_code()         # 纯函数,验证码提取(重点单测)
  └─ run_loop()             # 专用线程 2s 轮询(空闲退避到 4s)

src-tauri/src/commands/sms_code.rs           # 新增命令
  ├─ sms_code_status()      # 状态:未开启/无权限/运行中
  └─ open_notification_settings()  # 打开 ms-settings:privacy-notifications
```

设置面板加「短信验证码」区块(卡片样式与排除规则区一致):开关 + 权限状态 + 「打开系统设置」按钮 + 测试说明。

### 4.3 验证码提取算法(纯函数,可充分单测)

```rust
/// 从短信正文中提取验证码。返回 None = 不是验证码短信。
pub fn extract_code(body: &str) -> Option<String> {
    // 1. 关键词表(中英):验证码/校验码/动态码/识别码/code/OTP/PIN
    // 2. 候选:4-8 位连续数字(允许 "163-882" 这种分组,提取时去分隔符)
    // 3. 打分排序:
    //    + 关键词后 N 字符内出现(强信号)
    //    + 长度 4-6 优于 7-8
    //    - 前文是「尾号/卡号/账号」等负关键词(那是卡号不是验证码)
    //    - 11 位(手机号)或 13-19 位(卡号)直接排除
    //    - 前后紧邻其他数字(可能是长数字被切开)降分
    // 4. 最高分低于阈值 → None(宁可不提取,不写错验证码进剪贴板)
}
```

测试样例(实际收集中文短信的典型形态):

| 输入 | 期望输出 |
| --- | --- |
| `【淘宝】验证码:483920,您正在登录,请勿泄露` | `483920` |
| `Your WhatsApp code is 163-882` | `163882` |
| `【工商银行】您尾号3456的卡支出100元,余额5000元` | `None`(无关键词) |
| `【某某】验证码 1234,请在5分钟内输入。客服电话400-800-9000` | `1234`(不是 4008009000) |
| `【京东】动态密码:9 1 2 8,请勿告知他人` | `9128`(空格分隔) |
| `您的Apple ID验证码为:582913` | `582913` |

**设计原则:提取失败(返回 None)是安全默认值**——宁可少提取,不能把「卡号尾号」「订单金额」写进剪贴板覆盖用户正在复制的内容。

### 4.4 权限 UX(最大的坑,TeleLink 踩过)

`UserNotificationListener` 需要用户在 **设置 → 隐私和安全性 → 通知** 里把本应用加入「允许访问通知」的名单(TeleLink 的 SETUP 文档明确记录了这一步,且建议提权运行——我们本来就 requireAdministrator,反而占优)。

流程:
1. 用户第一次打开开关 → 后端 `RequestAccessAsync()`
2. 若返回 Denied → 前端弹引导卡片:「需要允许应用读取通知」,按钮直接 `start ms-settings:privacy-notifications`
3. 用户开完回来 → 前端轮询 `sms_code_status()` 显示 ✓

### 4.5 Rust 关键代码示例

```rust
use windows::core::Result;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::NotificationKinds;

fn ensure_access() -> Result<bool> {
    let listener = UserNotificationListener::Current()?;
    let status = listener.RequestAccessAsync()?.get()?; // 阻塞,放专用线程
    Ok(status == UserNotificationListenerAccessStatus::Allowed)
}

fn poll_once(listener: &UserNotificationListener) -> Result<Vec<ToastText>> {
    let notes = listener.GetNotificationsAsync(NotificationKinds::Toast)?.get()?;
    let mut out = Vec::new();
    for note in notes {
        // 过滤 Phone Link:AUMID 是语言无关的最可靠标识
        let aumid = note.AppInfo()?.AppUserModelId()?.to_lowercase();
        if !aumid.starts_with("microsoft.yourphone") { continue; }
        // 标题=发送者,后续 text elements 拼接=正文
        let visual = note.Notification()?.Visual()?;
        for binding in visual.Bindings()? {
            let texts = binding.GetTextElements()?;
            // texts[0] → sender, texts[1..] join → body
        }
        out.push(/* ... */);
    }
    Ok(out)
}
```

> 注:准确的方法签名以实现时 windows crate 的投影为准;`GetTextElements` 拿不到时降级解析 `Notification.Content().get_xml()`(照抄 TeleLink 的兜底)。

轮询线程(模式与现有 clipboard_monitor 一致):

```rust
pub fn spawn(app_handle: tauri::AppHandle) {
    std::thread::Builder::new().name("sms-code".into()).spawn(move || {
        loop {
            if is_enabled(&app_handle) {          // 设置开关
                if let Ok(toasts) = poll_once(&listener) {
                    for t in toasts {
                        if seen.insert(t.key()) { // (aumid, id) 去重
                            if let Some(code) = extract_code(&t.body) {
                                write_clipboard(&code);           // 不加 suppress
                                let _ = app_handle.emit("sms-code-copied",
                                    serde_json::json!({ "code": code, "sender": t.sender }));
                            }
                        }
                    }
                    seen.prune(&active_ids);       // 修剪已见集合
                }
            }
            std::thread::sleep(Duration::from_secs(2)); // 空闲退避 4s
        }
    });
}
```

Cargo.toml 增量:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.6x", features = [
    "UI_Notifications",
    "UI_Notifications_Management",
    "Foundation",
    "Foundation_Collections",
] }
```

---

## 5. 前置条件(用户侧设置)

### 安卓

1. 手机装「链接至 Windows」(Link to Windows),与 Phone Link 配对(用户已完成 ✅)
2. 手机上允许「链接至 Windows」读取短信和通知(配对流程里授权)
3. **省电策略把「链接至 Windows」加白名单**(国产 ROM 会杀后台,这是断流的头号原因)
4. Windows 通知设置里 Phone Link 的短信通知保持开启

### iPhone

1. 手机装「链接至 Windows」App,蓝牙配对(用户已完成 ✅)
2. iOS 设置 → 通知 → 链接至 Windows → **显示预览 = 始终**(预览关了正文是空的,提取不到)
3. iPhone 与 PC 需保持蓝牙连接;**锁屏不影响,但离开蓝牙范围就断**
4. 已知 Phone Link for iOS 限制:只转发「连接期间」收到的通知,无历史同步——对验证码场景无影响(验证码本来就是即时用的)

### PC(两平台通用)

1. Windows 设置 → 隐私和安全性 → 通知 → 允许本应用访问通知(应用内引导,一键跳转)
2. Phone Link 保持后台运行(默认开机自启)

---

## 6. 风险与注意事项

| # | 风险 | 等级 | 对策 |
| --- | --- | --- | --- |
| 1 | **通知监听权限被拒/被系统重置** | 高 | 状态命令实时检测,失权时暂停并在前端标红引导重开 |
| 2 | Phone Link 断连(蓝牙/后台被杀) | 高 | 轮询照跑,收到通知自然恢复;前端显示「最近一次捕获时间」让用户可感知 |
| 3 | iPhone 通知预览关闭 → 正文为空 | 中 | 检测到「Phone Link 有通知但正文全空」时提示用户去开预览 |
| 4 | 提取错误(把卡号尾号当验证码) | 中 | 负关键词表 + 阈值宁缺毋滥;单测覆盖典型中文短信 |
| 5 | 验证码覆盖用户正在复制的内容 | 低 | 仅在「内容命中验证码特征」时写剪贴板;写入前不弹确认(时效性优先),toast 告知 |
| 6 | 专注助手/勿扰模式 | 低 | banner 被抑制但通知仍进通知中心,listener 仍可读;实测确认 |
| 7 | Windows/Phone Link 更新改 AUMID 或行为 | 低 | 双重匹配(AUMID 前缀 + 显示名含 phone link/手机连接);本地依赖零,升级即修复 |
| 8 | **windows crate feature 增加编译时间** | 注意 | 与「不给 windows crate 加 feature」的项目约定冲突——但 WinRT 类 API **无法**用裸 `#[link]` 绕过,只能加 feature。feature 范围已压到最小(4 个);实现时实测增量编译耗时,若超预期再评估 |
| 9 | 隐私:短信通知内容经内存处理 | 低 | 全文不落盘、不写日志;只有验证码经剪贴板进历史;短信正文在本方案中不持久化。需在 SECURITY-AUDIT.md 补记 |
| 10 | PC 睡眠时短信到达 | 无法避免 | 睡眠期间无通知,唤醒后 Phone Link 若补转发则可捕获,否则错过(所有方案共同限制,含 Phone Link 本身) |

---

## 7. 实施计划(拍板后执行)

1. 切分支 `feat-sms-code`(**不带斜杠**,环境的坑已记录)
2. Cargo.toml 加 windows feature → 先写最小 demo 跑通「列出 Phone Link 通知」,验证权限/编译时间
3. `sms_code_service.rs` 完整实现 + `extract_code` 单测(Python 移植验证的老办法兜底 test 二进制起不来的问题)
4. 命令注册 + lib.rs 启动挂载(开关关闭时不轮询)
5. 前端:设置卡片 + 居中 toast + 系统通知复用
6. cargo check / clippy / tsc / 文档(README + SECURITY-AUDIT)
7. 分支提交推送,用户真机测试(需要真机收一条验证码短信)
