use serde::Serialize;

/// 版本信息文件主地址(GitHub raw)
const VERSION_JSON_URL: &str = "https://raw.githubusercontent.com/QikiiL/Clipboard/master/version.json";
/// 兜底镜像:jsdelivr 国内可达性较好;有约 12 小时缓存,最坏比 raw 晚半天看到新版本
const VERSION_JSON_FALLBACK_URL: &str =
    "https://cdn.jsdelivr.net/gh/QikiiL/Clipboard@master/version.json";

/// 读取 Windows 系统代理(Clash/V2Ray 等工具写入的 WinINET 设置)。
/// ureq 默认只认 HTTP_PROXY 环境变量、不读系统代理,而国内直连
/// raw.githubusercontent.com 经常超时——有系统代理时优先走它
fn system_http_proxy() -> Option<ureq::Proxy> {
    use winreg::enums::HKEY_CURRENT_USER;
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;
    let enabled: u32 = key.get_value("ProxyEnable").ok()?;
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    // 逐协议形式("http=127.0.0.1:7890;https=127.0.0.1:7890")优先取 https 段
    let server = if server.contains('=') {
        server
            .split(';')
            .find_map(|part| part.strip_prefix("https="))
            .or_else(|| server.split(';').find_map(|part| part.strip_prefix("http=")))
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else {
        server
    };
    if server.is_empty() {
        return None;
    }
    let url = if server.contains("://") {
        server
    } else {
        format!("http://{}", server)
    };
    ureq::Proxy::new(&url).ok()
}

fn fetch_manifest(url: &str, proxy: Option<&ureq::Proxy>) -> Result<UpdateManifest, String> {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(8));
    if let Some(p) = proxy {
        builder = builder.proxy(p.clone());
    }
    builder
        .build()
        .get(url)
        .call()
        .map_err(|e| format!("{}: {}", url, e))?
        .into_json()
        .map_err(|e| format!("解析版本信息失败: {}", e))
}

/// version.json 的结构:版本号 + 更新说明 + 两个手动下载页地址 + 蓝奏云密码
#[derive(serde::Deserialize, Clone)]
struct UpdateManifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    github: Option<String>,
    #[serde(default)]
    lanzou: Option<String>,
    #[serde(default)]
    lanzou_password: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub has_update: bool,
    pub notes: Option<String>,
    pub github: Option<String>,
    pub lanzou: Option<String>,
    pub lanzou_password: Option<String>,
}

fn version_tuple(v: &str) -> (u64, u64, u64) {
    let mut it = v
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// 检查更新:拉取 version.json 并与当前版本比较。
/// 必须是 async + spawn_blocking:同步命令在主线程执行,
/// 网络请求(最长 8 秒超时)会把整个窗口卡死;移到阻塞线程池后
/// 网络再慢也不影响 UI 响应。网络失败直接报错,由前端按"检查失败"展示。
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string());

    tauri::async_runtime::spawn_blocking(move || -> Result<UpdateInfo, String> {
        // 依次尝试:系统代理(若有) → 直连 → jsdelivr 镜像,第一个成功即用
        let proxy = system_http_proxy();
        let mut attempts: Vec<(&str, Option<&ureq::Proxy>)> = vec![(VERSION_JSON_URL, None)];
        if let Some(p) = proxy.as_ref() {
            attempts.insert(0, (VERSION_JSON_URL, Some(p)));
        }
        attempts.push((VERSION_JSON_FALLBACK_URL, proxy.as_ref()));

        let mut last_err = String::new();
        let mut manifest = None;
        for (url, p) in attempts {
            match fetch_manifest(url, p) {
                Ok(m) => {
                    manifest = Some(m);
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        let manifest = manifest
            .ok_or_else(|| format!("无法获取版本信息(代理/直连/镜像均失败): {}", last_err))?;

        Ok(UpdateInfo {
            has_update: version_tuple(&manifest.version) > version_tuple(&current),
            current,
            latest: manifest.version,
            notes: manifest.notes,
            github: manifest.github,
            lanzou: manifest.lanzou,
            lanzou_password: manifest.lanzou_password.filter(|p| !p.is_empty()),
        })
    })
    .await
    .map_err(|e| format!("更新检查任务失败: {}", e))?
}

/// 写入纯文本到系统剪贴板(复制蓝奏云密码用)
#[tauri::command]
pub fn write_clipboard_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text).map_err(|e| e.to_string())
}

/// 允许打开的下载域名,按**注册域后缀**匹配而非硬编码完整域名:
/// 蓝奏云有 lanzoul/lanzoui/lanzoux 等多个镜像域、且会随时切换,
/// version.json 今后也可能改用别的 CDN,只锁注册域才能容忍子域变化
const ALLOWED_DOWNLOAD_DOMAINS: &[&str] = &[
    "github.com",
    "jsdelivr.net",
    "lanzoul.com",
    "lanzoui.com",
    "lanzoux.com",
];

/// 取出 URL 的主机部分:先去掉 scheme,截到第一个 `/` 得到 authority,
/// 若含 userinfo 则取最后一个 `@` 之后的部分,再截掉 `:` 之后的端口。
/// 不引入 url crate——这里只需要这几行,多一个依赖就多一份攻击面
fn url_host(url: &str) -> Option<&str> {
    let after_scheme = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url,
    };
    let authority = after_scheme.split('/').next()?;
    let authority = match authority.rsplit_once('@') {
        Some((_, host_port)) => host_port,
        None => authority,
    };
    let host = authority.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

/// 主机是否落在白名单内。规则必须是 `host == domain || host.ends_with(".{domain}")`,
/// 少了那个点,`github.com.evil.com` 就会以"以 github.com 结尾"蒙混过关
fn host_is_allowed(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    ALLOWED_DOWNLOAD_DOMAINS.iter().any(|domain| {
        match host.strip_suffix(domain) {
            Some(rest) => rest.is_empty() || rest.ends_with('.'),
            None => false,
        }
    })
}

/// 在系统默认浏览器打开外部链接(更新下载页)
#[tauri::command]
pub fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    // 传入的 url 就是远程 version.json 里的 github/lanzou 字段,manifest
    // 一旦被篡改就能把用户导向钓鱼下载页,所以这里强制 https + 域名白名单,
    // 校验失败直接拒绝打开(而不是静默放行或降级处理)
    let scheme = url
        .split_once("://")
        .map(|(s, _)| s.to_ascii_lowercase())
        .unwrap_or_default();
    let host_allowed = url_host(&url).map(host_is_allowed).unwrap_or(false);
    if scheme != "https" || !host_allowed {
        return Err(format!(
            "已拒绝打开该链接:仅允许 https 协议的官方下载域名({})",
            ALLOWED_DOWNLOAD_DOMAINS.join("、")
        ));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
