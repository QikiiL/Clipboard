use serde::Serialize;

/// 版本信息文件地址:发版时把仓库根目录的 version.json 上传到任意
/// 可公开访问的静态位置(GitHub raw / Gitee / 对象存储均可),并更新此常量。
const VERSION_JSON_URL: &str = "https://raw.githubusercontent.com/EXAMPLE/clipboard/main/version.json";

/// version.json 的结构:版本号 + 更新说明 + 两个手动下载页地址
#[derive(serde::Deserialize, Clone)]
struct UpdateManifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    github: Option<String>,
    #[serde(default)]
    lanzou: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub has_update: bool,
    pub notes: Option<String>,
    pub github: Option<String>,
    pub lanzou: Option<String>,
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
/// 网络失败直接报错,由前端按“检查失败”展示。
#[tauri::command]
pub fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let manifest: UpdateManifest = ureq::get(VERSION_JSON_URL)
        .timeout(std::time::Duration::from_secs(8))
        .call()
        .map_err(|e| format!("无法获取版本信息: {}", e))?
        .into_json()
        .map_err(|e| format!("解析版本信息失败: {}", e))?;

    let current = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string());
    Ok(UpdateInfo {
        has_update: version_tuple(&manifest.version) > version_tuple(&current),
        current,
        latest: manifest.version,
        notes: manifest.notes,
        github: manifest.github,
        lanzou: manifest.lanzou,
    })
}

/// 在系统默认浏览器打开外部链接(更新下载页)
#[tauri::command]
pub fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
