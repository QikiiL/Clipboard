use std::path::{Path, PathBuf};
use tauri::Manager;

// 数据存储位置管理。
// 数据(clipboard.db + images/)默认在应用配置目录,用户可在设置中改为任意目录。
// 指针保存在默认配置目录的 storage.json(引导文件),必须在任何数据库连接
// 建立之前读取。迁移采用两阶段:先写指针并重启,冷启动时(无任何连接占用)
// 搬运文件并改写数据库里的图片绝对路径,避免复制运行中的 SQLite 文件。

const BOOTSTRAP_FILE: &str = "storage.json";

/// canonicalize() 在 Windows 返回 `\\?\` 前缀的扩展路径;它拼进 sqlite: URL
/// 会产生非法连接串(sqlite://?/D:/...)导致启动即崩,持久化前必须去掉
fn strip_extended_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", rest))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// 启动时解析一次的数据目录,之后全程只读
pub struct StorageDir(pub PathBuf);

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct StorageBootstrap {
    /// 自定义数据目录;None = 默认位置
    #[serde(default)]
    data_dir: Option<String>,
    /// 待迁移的旧目录;改位置后首次重启时搬运,完成后清除
    #[serde(default)]
    migrate_from: Option<String>,
}

fn config_dir() -> PathBuf {
    crate::app_config_dir()
}

fn default_data_dir() -> PathBuf {
    crate::app_data_dir_default()
}

fn bootstrap_path() -> PathBuf {
    config_dir().join(BOOTSTRAP_FILE)
}

fn read_bootstrap() -> StorageBootstrap {
    std::fs::read_to_string(bootstrap_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_bootstrap(b: &StorageBootstrap) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录: {}", e))?;
    let json = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(bootstrap_path(), json).map_err(|e| format!("写入存储位置失败: {}", e))
}

/// 当前生效的数据目录(state 优先,兜底默认)
pub fn current_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.try_state::<StorageDir>()
        .map(|d| d.0.clone())
        .unwrap_or_else(default_data_dir)
}

/// 统一的 images 目录解析(所有图片读写/校验都必须走这里,
/// 保证自定义存储位置生效)
pub fn images_dir(app: &tauri::AppHandle) -> PathBuf {
    current_data_dir(app).join("images")
}

/// 校验图片路径位于应用 images 目录内,返回规范化的绝对路径;越界或无法解析返回 None。
/// 图片的 file_path 来自数据库条目,可能指向 images 之外的任意文件,
/// 因此读取、删除、粘贴三条路径都必须先过这道校验,否则任一处的守卫缺口
/// 就足以让被篡改的条目打开磁盘上的任意图片。
///
/// 注意:返回值仅供内部比较与打开使用,不要写入数据库或其它持久化位置——
/// Windows 的 canonicalize 会带 `\\?\` 前缀(见 strip_extended_prefix)。
pub fn resolve_image_path(app: &tauri::AppHandle, file_path: &str) -> Option<PathBuf> {
    let images_dir = images_dir(app);
    // 全新安装后 images 目录可能还没建过,先建再 canonicalize,否则校验恒失败
    std::fs::create_dir_all(&images_dir).ok()?;
    let canonical_images = std::fs::canonicalize(&images_dir).ok()?;
    let canonical = std::fs::canonicalize(file_path).ok()?;
    if canonical.starts_with(&canonical_images) {
        Some(canonical)
    } else {
        None
    }
}

/// 启动入口:解析数据目录;若存在待迁移项,在无连接状态下搬运,
/// 任一步失败则回退旧目录(数据不动)。
pub fn resolve_data_dir() -> PathBuf {
    let bootstrap = read_bootstrap();
    let default = default_data_dir();
    // 读取时同样剥离 \\?\ 前缀,自愈旧版本写入的扩展路径
    let target = bootstrap
        .data_dir
        .as_ref()
        .map(|s| strip_extended_prefix(Path::new(s)))
        .unwrap_or_else(|| default.clone());

    if let Some(from) = bootstrap.migrate_from.clone() {
        let from = strip_extended_prefix(Path::new(&from));
        match migrate_data(&from, &target) {
            Ok(()) => {
                let mut b = bootstrap.clone();
                b.migrate_from = None;
                let _ = write_bootstrap(&b);
            }
            Err(e) => {
                // 迁移失败:继续用旧目录,保证数据可用
                eprintln!(
                    "Storage migration to {} failed ({}), keep using {}",
                    target.display(),
                    e,
                    from.display()
                );
                let mut b = bootstrap.clone();
                b.data_dir = Some(from.to_string_lossy().to_string());
                b.migrate_from = None;
                let _ = write_bootstrap(&b);
                return from;
            }
        }
    }

    // 自定义目录当前不可用(如移动盘被拔)时兜底默认目录
    if target != default && std::fs::create_dir_all(&target).is_err() {
        eprintln!(
            "Custom data dir {} unavailable, fallback to default",
            target.display()
        );
        return default;
    }
    target
}

/// 搬运 from → to:数据库(含 WAL 伴生文件)+ images/,
/// 并改写库内图片绝对路径;全部成功后删除旧位置的数据文件。
fn migrate_data(from: &Path, to: &Path) -> Result<(), String> {
    let from_db = from.join("clipboard.db");
    if !from_db.exists() {
        // 旧位置没有数据(全新安装后改位置),只需保证新目录存在
        std::fs::create_dir_all(to).map_err(|e| format!("创建目录失败: {}", e))?;
        return Ok(());
    }
    if to.join("clipboard.db").exists() {
        return Err("目标位置已存在剪贴板数据".to_string());
    }
    std::fs::create_dir_all(to).map_err(|e| format!("创建目录失败: {}", e))?;

    for name in ["clipboard.db", "clipboard.db-wal", "clipboard.db-shm"] {
        let src = from.join(name);
        if src.exists() {
            std::fs::copy(&src, to.join(name)).map_err(|e| format!("复制 {} 失败: {}", name, e))?;
        }
    }

    let from_images = from.join("images");
    if from_images.is_dir() {
        let to_images = to.join("images");
        std::fs::create_dir_all(&to_images).map_err(|e| format!("创建图片目录失败: {}", e))?;
        // 图片目录是平铺的单层 PNG,不递归
        for entry in std::fs::read_dir(&from_images).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().is_file() {
                let file_name = entry.file_name();
                std::fs::copy(entry.path(), to_images.join(&file_name))
                    .map_err(|e| format!("复制图片 {} 失败: {}", file_name.to_string_lossy(), e))?;
            }
        }
    }

    rewrite_image_paths(&to.join("clipboard.db"), from, to)?;

    // 新库已成功打开并完成路径改写,旧位置数据可以清理
    // (只删数据文件;默认配置目录里的 settings.json 等不受影响)
    for name in ["clipboard.db", "clipboard.db-wal", "clipboard.db-shm"] {
        let _ = std::fs::remove_file(from.join(name));
    }
    let _ = std::fs::remove_dir_all(from.join("images"));
    Ok(())
}

/// 把数据库里以旧目录为前缀的图片绝对路径改写到新目录
fn rewrite_image_paths(db_location: &Path, from: &Path, to: &Path) -> Result<(), String> {
    let from_native = from.to_string_lossy().to_string();
    let from_fwd = from_native.replace('\\', "/");
    let to_native = to.to_string_lossy().to_string();

    tauri::async_runtime::block_on(async {
        let options = sqlx::sqlite::SqliteConnectOptions::new().filename(db_location);
        let pool = sqlx::sqlite::SqlitePool::connect_with(options)
            .await
            .map_err(|e| format!("打开迁移后的数据库失败: {}", e))?;

        let rows: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, file_path FROM items WHERE file_path IS NOT NULL")
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("读取图片路径失败: {}", e))?;

        for (id, fp) in rows {
            let rest = fp
                .strip_prefix(&from_native)
                .or_else(|| fp.strip_prefix(&from_fwd))
                .map(|r| r.trim_start_matches(['\\', '/']).to_string());
            if let Some(rest) = rest {
                let new_path = Path::new(&to_native).join(&rest);
                sqlx::query("UPDATE items SET file_path = ? WHERE id = ?")
                    .bind(new_path.to_string_lossy().to_string())
                    .bind(id)
                    .execute(&pool)
                    .await
                    .map_err(|e| format!("改写图片路径失败: {}", e))?;
            }
        }
        pool.close().await;
        Ok::<(), String>(())
    })
}

/// 校验并把存储位置切到 new_dir(写引导文件,不立即搬文件)。
/// 实际迁移在重启后的冷启动里完成。
fn apply_storage_change(app: &tauri::AppHandle, new_dir: PathBuf) -> Result<(), String> {
    let current = current_data_dir(app);
    let cur = std::fs::canonicalize(&current).unwrap_or_else(|_| current.clone());
    // 目标目录可能尚不存在(恢复默认时 data 文件夹可能从没建过),
    // 必须先创建再 canonicalize,否则报“无法访问所选位置”
    std::fs::create_dir_all(&new_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let new = std::fs::canonicalize(&new_dir)
        .map_err(|e| format!("无法访问所选位置: {}", e))?;

    if cur == new {
        return Err("新位置与当前存储位置相同".to_string());
    }
    if new.starts_with(&cur) || cur.starts_with(&new) {
        return Err("新位置不能与当前位置互相嵌套".to_string());
    }
    if new.join("clipboard.db").exists() || new.join("images").exists() {
        return Err("所选文件夹已包含剪贴板数据,请选择空文件夹".to_string());
    }
    std::fs::create_dir_all(&new).map_err(|e| format!("创建目录失败: {}", e))?;

    // 写权限探测(拒绝只读/受限位置)
    let probe = new.join(".clipboard-write-test");
    std::fs::write(&probe, b"t")
        .and_then(|_| std::fs::remove_file(&probe))
        .map_err(|e| format!("所选位置不可写: {}", e))?;

    let mut b = read_bootstrap();
    // 存储去掉 \\?\ 前缀的普通路径(extended 路径会破坏 sqlite: URL)
    b.data_dir = Some(strip_extended_prefix(&new).to_string_lossy().to_string());
    b.migrate_from = Some(current.to_string_lossy().to_string());
    write_bootstrap(&b)
}

#[derive(serde::Serialize)]
pub struct StorageInfo {
    pub data_dir: String,
    pub is_default: bool,
    pub default_dir: String,
}

#[tauri::command]
pub fn get_storage_info(app: tauri::AppHandle) -> Result<StorageInfo, String> {
    let data_dir = current_data_dir(&app);
    let default_dir = default_data_dir();
    Ok(StorageInfo {
        data_dir: data_dir.to_string_lossy().to_string(),
        is_default: data_dir == default_dir,
        default_dir: default_dir.to_string_lossy().to_string(),
    })
}

/// 更改存储位置:原生目录选择器 → 校验 → 写引导文件 → 重启,
/// 重启后冷启动完成数据搬运
#[tauri::command]
pub async fn change_storage_location(app: tauri::AppHandle) -> Result<(), String> {
    let app_for_picker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        app_for_picker.dialog().file().blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(picked) = picked else {
        return Ok(()); // 用户取消选择
    };
    let path: PathBuf = picked
        .into_path()
        .map_err(|e| format!("解析所选路径失败: {}", e))?;

    apply_storage_change(&app, path)?;
    app.restart();
}

/// 恢复默认存储位置(同样走重启 + 冷启动迁移)
#[tauri::command]
pub fn reset_storage_location(app: tauri::AppHandle) -> Result<(), String> {
    apply_storage_change(&app, default_data_dir())?;
    app.restart();
}
