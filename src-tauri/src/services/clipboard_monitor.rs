use crate::models::clipboard_type::ClipboardType;
use crate::services::exclusion_service::{ExclusionCheck, ExclusionReason, ExclusionState};
use crate::utils::hash::{compute_hash, compute_hash_bytes};
use arboard::Clipboard as ArboardClipboard;
use image::RgbaImage;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Mutex;

/// Fallback: read CF_DIB from the Windows clipboard directly and convert to RGBA.
/// Returns (rgba_bytes, width, height) or None.
fn read_clipboard_dib_fallback() -> Option<(Vec<u8>, usize, usize)> {
    use clipboard_win::formats::RawData;

    let _clip = clipboard_win::Clipboard::new().ok()?;

    // CF_DIB = 8
    const CF_DIB: u32 = 8;
    if !clipboard_win::is_format_avail(CF_DIB) {
        return None;
    }

    let dib_data: Vec<u8> = clipboard_win::get(RawData(CF_DIB)).ok()?;
    if dib_data.len() < 36 {
        return None;
    }

    // Read BITMAPINFOHEADER size from first 4 bytes
    let header_size =
        u32::from_le_bytes([dib_data[0], dib_data[1], dib_data[2], dib_data[3]]) as usize;

    // 只处理 BITMAPINFOHEADER 及其扩展(≥40 字节,前 40 字节布局一致):
    // BITMAPCOREHEADER(12)等老格式的 biBitCount/biClrUsed 偏移不同,
    // 按错误偏移解析只会得到垃圾数据,宁可放弃这一帧
    if header_size < 40 {
        return None;
    }

    // Read biBitCount (offset 14) and biClrUsed (offset 32) for color table size
    let bit_count = u16::from_le_bytes([dib_data[14], dib_data[15]]);
    let clr_used =
        u32::from_le_bytes([dib_data[32], dib_data[33], dib_data[34], dib_data[35]]) as usize;
    let color_table_size = if clr_used > 0 {
        clr_used * 4
    } else if bit_count <= 8 {
        (1usize << bit_count) * 4
    } else {
        0
    };

    // Prepend a 14-byte BMP file header
    let file_size = 14u32 + dib_data.len() as u32;
    let pixel_offset = 14u32 + header_size as u32 + color_table_size as u32;
    let mut bmp_data = Vec::with_capacity(file_size as usize);
    bmp_data.extend_from_slice(b"BM");
    bmp_data.extend_from_slice(&file_size.to_le_bytes());
    bmp_data.extend_from_slice(&[0u8; 4]); // reserved
    bmp_data.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp_data.extend_from_slice(&dib_data);

    let img = image::load_from_memory(&bmp_data).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    Some((rgba.into_raw(), width, height))
}

/// Check if a string looks like a Windows file path (drive letter or UNC path).
/// 不再把 `/` 前缀的文本判为文件:那是 Unix 语法,在 Windows 上不是有效路径,
/// 误判会让粘贴走 CF_HDROP 写入无效文件列表而失败,展示图标也是误导。
fn is_file_path(s: &str) -> bool {
    // Windows drive letter (C:\...)
    (s.len() >= 3
        && s.as_bytes()[1] == b':'
        && (s.as_bytes()[2] == b'\\' || s.as_bytes()[2] == b'/'))
    // UNC path (\\server\share)
    || (s.starts_with("\\\\") && s.len() > 2)
}

/// Detect content type from text content.
fn detect_content_type(text: &str) -> i32 {
    if text.starts_with("http://") || text.starts_with("https://") {
        ClipboardType::Link as i32
    } else if is_file_path(text) {
        ClipboardType::File as i32
    } else {
        ClipboardType::Text as i32
    }
}

/// 一次剪贴板抓取的结果
struct CapturedContent {
    content: String,
    item_type: i32,
    image_hash: Option<String>,
    image_bytes: Option<Vec<u8>>,
    width: usize,
    height: usize,
    /// Windows 扩展格式的原始字节(混排内容):"HTML Format" / "Rich Text Format"。
    /// 保留 CF_HTML 头部原样、不解析不重构,保证恢复时无损往返。
    /// HDROP/纯图片路径填 None(纯图片位图已完整保留图片,不在本 bug 范围)。
    html_bytes: Option<Vec<u8>>,
    rtf_bytes: Option<Vec<u8>>,
    /// 放置这段内容的来源进程名(小写);判定不了时为 None。
    /// 必须在打开剪贴板之前取:剪贴板是独占资源,打开后 GetClipboardOwner 会失败
    source_process: Option<String>,
}

/// 文本类条目的组合去重 hash:(text, html_bytes, rtf_bytes) 依次喂给 SHA-256,
/// 中间加分隔符避免相邻字段拼接歧义。
/// 无 html/rtf 时必须与 compute_hash(text) 完全一致 —— 否则升级后旧条目
/// 与同一内容的纯文本新捕获 hash 不同,会被当成两条重复入库。
fn compute_text_combo_hash(text: &str, html: Option<&[u8]>, rtf: Option<&[u8]>) -> String {
    match (html, rtf) {
        (None, None) => compute_hash(text),
        _ => {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            // 分隔符用不易出现在正常内容里的字节序列
            hasher.update(b"\x00--fmt--\x00");
            if let Some(h) = html {
                hasher.update(h);
            }
            hasher.update(b"\x00--fmt--\x00");
            if let Some(r) = rtf {
                hasher.update(r);
            }
            format!("{:x}", hasher.finalize())
        }
    }
}

/// 组装文本类捕获结果(纯逻辑,供 read_clipboard_content 复用与单测覆盖)。
/// html/rtf 缺失(None)时产物与旧路径逐字段一致。
fn assemble_text_capture(
    text: String,
    item_type: i32,
    html_bytes: Option<Vec<u8>>,
    rtf_bytes: Option<Vec<u8>>,
    source_process: Option<String>,
) -> CapturedContent {
    CapturedContent {
        content: text,
        item_type,
        image_hash: None,
        image_bytes: None,
        width: 0,
        height: 0,
        html_bytes,
        rtf_bytes,
        source_process,
    }
}

/// 单次捕获读到的文本 + 扩展格式原始字节
struct TextWithFormats {
    text: String,
    html: Option<Vec<u8>>,
    rtf: Option<Vec<u8>>,
}

/// 单个 clipboard-win 会话内读取文本 + "HTML Format" + "Rich Text Format"。
/// 关键:GetClipboardData 必须 OpenClipboard 后才能成功——clipboard_win::get()
/// 系列不打开剪贴板,必须在持有 Clipboard guard 的作用域内调用。
/// 文本用 clipboard_win::formats::Unicode 读(与 arboard get_text 在 Windows
/// 上同实现:raw::get_string,含尾 NUL 剥离,行为逐字节一致)。
/// 同会话读三格式还消除了"读文本后、读格式前剪贴板被改写"的 TOCTOU。
/// 返回 None = 文本不可用(与旧路径语义一致);html/rtf 读不到则为 None。
fn read_text_with_extended_formats() -> Option<TextWithFormats> {
    use clipboard_win::formats::{RawData, Unicode};

    // 打不开剪贴板(被其他进程占用)时本轮什么都读不到,下一轮再试
    let _clip = clipboard_win::Clipboard::new_attempts(10).ok()?;

    // 文本必须可用:与旧路径一致,纯文本是条目的存在基础
    if !clipboard_win::is_format_avail(clipboard_win::formats::CF_UNICODETEXT) {
        return None;
    }
    let text: String = clipboard_win::get(Unicode).ok()?;
    if text.is_empty() {
        return None;
    }

    // 扩展格式:格式不可用或读取失败返回 None(原始字节含 CF_HTML 头原样保留)
    fn read_named_format(name: &str) -> Option<Vec<u8>> {
        let fmt = clipboard_win::raw::register_format(name)?;
        let data: Vec<u8> = clipboard_win::get(RawData(fmt.get())).ok()?;
        if data.is_empty() {
            None
        } else {
            Some(data)
        }
    }
    let html = read_named_format("HTML Format");
    let rtf = read_named_format("Rich Text Format");

    Some(TextWithFormats { text, html, rtf })
}

/// Read clipboard content on a blocking thread.
/// 返回 None 表示剪贴板为空或读取失败。
fn read_clipboard_content() -> Option<CapturedContent> {
    // 来源进程要在打开剪贴板之前取(打开后 GetClipboardOwner 拿不到 owner)。
    // 这是阻塞式 Win32 调用,而本函数本就跑在 spawn_blocking 里
    let source_process = crate::services::exclusion_service::source_process_name();

    // Try reading a file list (CF_HDROP) first — copied from Explorer.
    // The clipboard_win guard must be dropped before arboard calls below,
    // as both open the clipboard exclusively.
    {
        // 打不开剪贴板(被其他进程占用)时本轮什么都读不到,下一轮再试;
        // 多试几次以减少偶发冲突导致的整轮空转
        let _clip = clipboard_win::Clipboard::new_attempts(10).ok()?;
        const CF_HDROP: u32 = 15;
        if clipboard_win::is_format_avail(CF_HDROP) {
            // HDROP 读取失败(或列表为空)不能中断本轮:复制方可能处于延迟渲染
            // 等瞬态,直接返回 None 会让图片/文本捕获被一条坏 HDROP 卡住
            let files = clipboard_win::get::<Vec<String>, _>(clipboard_win::formats::FileList);
            if let Ok(files) = files {
                if !files.is_empty() {
                    return Some(CapturedContent {
                        content: files.join("\n"),
                        item_type: ClipboardType::File as i32,
                        image_hash: None,
                        image_bytes: None,
                        width: 0,
                        height: 0,
                        html_bytes: None,
                        rtf_bytes: None,
                        source_process,
                    });
                }
            }
        }
    }

    let mut cb = ArboardClipboard::new().ok()?;

    // Try reading an image first
    match cb.get_image() {
        Ok(img) => {
            let bytes = img.bytes.to_vec();
            let width = img.width;
            let height = img.height;
            let hash = compute_hash_bytes(&bytes);
            return Some(CapturedContent {
                content: "[图片]".to_string(),
                item_type: ClipboardType::Image as i32,
                image_hash: Some(hash),
                image_bytes: Some(bytes),
                width,
                height,
                // 纯图片路径行为一字不改:位图已完整保留图片,不读扩展格式
                html_bytes: None,
                rtf_bytes: None,
                source_process,
            });
        }
        Err(_) => {
            // Try DIB fallback (Windows-specific)
            if let Some((bytes, width, height)) = read_clipboard_dib_fallback() {
                let hash = compute_hash_bytes(&bytes);
                return Some(CapturedContent {
                    content: "[图片]".to_string(),
                    item_type: ClipboardType::Image as i32,
                    image_hash: Some(hash),
                    image_bytes: Some(bytes),
                    width,
                    height,
                    html_bytes: None,
                    rtf_bytes: None,
                    source_process,
                });
            }
        }
    }

    // Try reading text + extended formats (HTML/RTF) in ONE clipboard-win session.
    // 图文混排内容:文字之外通常同时带 "HTML Format"/"Rich Text Format"
    // (图片内嵌其中、无独立 CF_DIB)。上方的 arboard 调用是即开即关、
    // 不长期持锁;这里持 clipboard-win guard 是 GetClipboardData 能成功的
    // 前提(剪贴板独占打开,不持 guard 的 GetClipboardData 必然失败)。
    match read_text_with_extended_formats() {
        Some(TextWithFormats {
            text,
            html: html_bytes,
            rtf: rtf_bytes,
        }) => {
            let item_type = detect_content_type(&text);
            Some(assemble_text_capture(
                text, item_type, html_bytes, rtf_bytes, source_process,
            ))
        }
        None => None,
    }
}

/// 排除原因的事件标识,供前端区分提示文案
fn reason_str(reason: Option<ExclusionReason>) -> &'static str {
    match reason {
        Some(ExclusionReason::App) => "app",
        Some(ExclusionReason::Pattern) => "pattern",
        Some(ExclusionReason::Sensitive) => "sensitive",
        None => "unknown",
    }
}

/// 排除事件负载。带上 hash 是为了让前端「仍要记录」按钮知道该豁免哪一条
#[derive(Clone, serde::Serialize)]
struct ExcludedEvent {
    reason: &'static str,
    hash: String,
}

/// Save image bytes as a PNG file to the app's images directory.
fn save_image_to_disk(
    app_handle: &tauri::AppHandle,
    hash: &str,
    bytes: &[u8],
    width: usize,
    height: usize,
) -> Option<String> {
    let images_dir = crate::services::storage_service::images_dir(app_handle);
    if let Err(e) = std::fs::create_dir_all(&images_dir) {
        eprintln!("Failed to create images directory: {}", e);
    }
    let file_name = format!("{}.png", &hash[..hash.len().min(16)]);
    let path = images_dir.join(&file_name);
    if let Some(img_buf) = RgbaImage::from_raw(width as u32, height as u32, bytes.to_vec()) {
        // Convert to RGB to strip alpha channel and color profile metadata.
        // This prevents the "iCCP: cHRM chunk does not match sRGB" libpng warning.
        let dynamic_img = image::DynamicImage::ImageRgba8(img_buf);
        let rgb_img = dynamic_img.to_rgb8();
        if rgb_img.save(&path).is_ok() {
            Some(path.to_string_lossy().to_string())
        } else {
            eprintln!("Failed to save image to {:?}", path);
            None
        }
    } else {
        eprintln!(
            "Failed to create RgbaImage from raw bytes: dimensions={}x{}, byte_count={}",
            width,
            height,
            bytes.len()
        );
        None
    }
}

/// 抑制 TTL:正常投递在 set_suppress(true) 后 ≤1s 内显式解除
/// (单击路径 500ms,队列步进更短);TTL 只是投递卡死时的兜底
const SUPPRESS_TTL_MS: u64 = 2000;

/// 把富文本格式的原始字节写入 formats 目录。
/// 文件名用组合 hash 前 16 字符 + 扩展名。失败返回 None:
/// 与图片落盘失败不同,这里不丢整个条目 —— 调用方降级为纯文本入库。
fn save_format_to_disk(
    app_handle: &tauri::AppHandle,
    hash: &str,
    ext: &str,
    bytes: &[u8],
) -> Option<String> {
    let formats_dir = crate::services::storage_service::formats_dir(app_handle);
    if let Err(e) = std::fs::create_dir_all(&formats_dir) {
        eprintln!("Failed to create formats directory: {}", e);
        return None;
    }
    let file_name = format!("{}.{}", &hash[..hash.len().min(16)], ext);
    let path = formats_dir.join(&file_name);
    std::fs::write(&path, bytes)
        .map_err(|e| eprintln!("Failed to save format file {:?}: {}", path, e))
        .ok()?;
    Some(path.to_string_lossy().to_string())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct ClipboardMonitor {
    /// 投递期间抑制采集的截止时间(毫秒时间戳,0 = 未抑制)。带 TTL:
    /// 旧实现是无限期 bool,投递链路一旦卡死(剪贴板被其他进程长期占用等),
    /// 末尾的复位代码永远执行不到,采集从此静默 —— 表现为“软件坏了,
    /// 必须重启才能恢复记录”。改成截止时间后最多 SUPPRESS_TTL_MS 自愈
    suppress_until: Arc<AtomicU64>,
    paused: Arc<Mutex<bool>>,
    last_hash: Arc<Mutex<String>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            suppress_until: Arc::new(AtomicU64::new(0)),
            paused: Arc::new(Mutex::new(false)),
            last_hash: Arc::new(Mutex::new(String::new())),
        }
    }

    /// true = 抑制采集(TTL 内自动过期兜底),false = 立即解除
    pub async fn set_suppress(&self, value: bool) {
        let until = if value { now_ms() + SUPPRESS_TTL_MS } else { 0 };
        self.suppress_until.store(until, Ordering::SeqCst);
    }

    /// 投递热点路径(每次轮询、写入前都要查),同步原子读即可
    pub fn is_suppressed(&self) -> bool {
        let until = self.suppress_until.load(Ordering::SeqCst);
        until != 0 && now_ms() < until
    }

    pub async fn set_paused(&self, value: bool) {
        let mut paused = self.paused.lock().await;
        *paused = value;
    }

    pub async fn is_paused(&self) -> bool {
        *self.paused.lock().await
    }

    pub async fn toggle_paused(&self) {
        let mut paused = self.paused.lock().await;
        *paused = !*paused;
    }

    /// Start polling the system clipboard for changes.
    pub fn start_polling(app_handle: tauri::AppHandle, db: SqlitePool, monitor: ClipboardMonitor) {
        let monitor = monitor.clone();
        tauri::async_runtime::spawn(async move {
            // 规则状态在循环外取一次:state 内部是 Arc,持有它不会阻塞设置刷新,
            // 也省掉每 500ms 一次的状态查找
            let exclusion = app_handle.try_state::<ExclusionState>();

            // 播种:启动时先读一次当前剪贴板,把内容 hash 记入 last_hash 但**不入库**。
            // 不播种的话,首轮轮询会把"启动前就留在剪贴板里的内容"重新入库:
            // 库里有同条则被 UPDATE last_used_at 顶到最上面(时间显示"刚刚"),
            // 用户删掉记录后重启还会再次生成 —— 表现为"每次打开软件都重复记录
            // 同一条"。启动后新复制的内容 hash 不同,照常捕获;软件未运行期间的
            // 复制本来就不入历史,与短信捕获的会话起点语义一致
            let seed = tokio::task::spawn_blocking(read_clipboard_content)
                .await
                .unwrap_or(None);
            if let Some(captured) = seed {
                let seed_hash = if let Some(ih) = captured.image_hash {
                    ih
                } else {
                    compute_text_combo_hash(
                        &captured.content,
                        captured.html_bytes.as_deref(),
                        captured.rtf_bytes.as_deref(),
                    )
                };
                *monitor.last_hash.lock().await = seed_hash;
            }

            // 启动捕获由下方轮询循环完成:tokio interval 的首次 tick 立即到期,
            // 与后续读取共用同一套去重/入库/发事件逻辑(独一份,避免双份维护)。
            // 首轮读到的是播种过的内容,直接跳过;真正的启动捕获只发生在
            // 软件运行期间发生的新复制
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                if monitor.is_suppressed() || *monitor.paused.lock().await {
                    continue;
                }

                // Read clipboard on a blocking thread to avoid arboard deadlocks
                let clipboard_result = tokio::task::spawn_blocking(read_clipboard_content)
                    .await
                    .unwrap_or(None);

                let Some(captured) = clipboard_result else {
                    continue;
                };
                let CapturedContent {
                    content,
                    item_type,
                    image_hash,
                    image_bytes,
                    width: img_width,
                    height: img_height,
                    html_bytes,
                    rtf_bytes,
                    source_process,
                } = captured;

                let hash = if let Some(ih) = image_hash {
                    ih
                } else {
                    compute_text_combo_hash(&content, html_bytes.as_deref(), rtf_bytes.as_deref())
                };

                // 用户在「已排除」提示上点了「仍要记录」:清空 last_hash,让本轮
                // 重新走一遍入库流程。内容此刻仍在系统剪贴板里,不需要用户再复制
                // 一次。信号取走即清,只可能影响紧接着的这一轮
                if let Some(state) = &exclusion {
                    if state.take_recapture().as_deref() == Some(hash.as_str()) {
                        *monitor.last_hash.lock().await = String::new();
                    }
                }

                // Skip if same as last
                {
                    let last = monitor.last_hash.lock().await;
                    if *last == hash {
                        continue;
                    }
                }

                // Skip if suppress/paused changed during blocking read
                if monitor.is_suppressed() || *monitor.paused.lock().await {
                    continue;
                }

                // 排除规则:命中则不入库,密码、密钥之类不落盘
                if let Some(state) = &exclusion {
                    // 用户已豁免的内容直接放行,不再重跑规则
                    let check = if state.is_allowed(&hash) {
                        ExclusionCheck {
                            excluded: false,
                            reason: None,
                        }
                    } else {
                        state.check(&content, item_type, source_process.as_deref())
                    };
                    if check.excluded {
                        // 也要把 hash 记进 last_hash:被排除的内容会一直留在系统
                        // 剪贴板里,不记的话每 500ms 都要对同一份内容重跑一遍
                        // 全部正则。代价是此后用户删掉该规则、再复制同一份内容
                        // 时不会被记录 —— 这个代价现在由「仍要记录」按钮兜底
                        *monitor.last_hash.lock().await = hash.clone();
                        let _ = app_handle.emit(
                            "item-excluded",
                            ExcludedEvent {
                                reason: reason_str(check.reason),
                                hash: hash.clone(),
                            },
                        );
                        // Windows 系统通知:仅在主窗口未获焦时发 —— 获焦时
                        // 应用内已经弹了居中 toast,再发系统通知就是重复打扰
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if !window.is_focused().unwrap_or(false) {
                                let _ = app_handle
                                    .notification()
                                    .builder()
                                    .title("剪贴板内容已拦截")
                                    .body("可能复制到了密码、密钥或卡号，已自动丢弃。打开应用确认。")
                                    .show();
                            }
                        }
                        continue;
                    }
                }

                let preview: String = content.chars().take(100).collect();

                // Save image bytes to file if it's an image
                let file_path: Option<String> = if item_type == ClipboardType::Image as i32 {
                    image_bytes.as_deref().and_then(|bytes| {
                        save_image_to_disk(&app_handle, &hash, bytes, img_width, img_height)
                    })
                } else {
                    None
                };

                // html/rtf 原始字节落盘(仅文本类条目携带)。落盘失败降级:
                // 条目照常入库、两列置 NULL,恢复时退化为纯文本,不丢条目
                let (html_path, rtf_path) = if item_type == ClipboardType::Image as i32 {
                    (None, None)
                } else {
                    (
                        html_bytes
                            .as_deref()
                            .and_then(|b| save_format_to_disk(&app_handle, &hash, "html", b)),
                        rtf_bytes
                            .as_deref()
                            .and_then(|b| save_format_to_disk(&app_handle, &hash, "rtf", b)),
                    )
                };

                // Update last hash
                {
                    let mut last = monitor.last_hash.lock().await;
                    *last = hash.clone();
                }

                // 图片落盘失败(磁盘满/尺寸与字节数不匹配)时不入库:留下一条
                // 无图的 "[图片]" 死条目既显示不出、粘贴也会被路径守卫拒绝。
                // last_hash 已在上面更新,不会对同一帧图片反复重试
                if item_type == ClipboardType::Image as i32 && file_path.is_none() {
                    continue;
                }

                // Check for existing item
                let existing = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM items WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(&db)
                .await;

                match existing {
                    Ok(Some(id)) => {
                        let _ = sqlx::query(
                            "UPDATE items SET last_used_at = datetime('now') WHERE id = ?",
                        )
                        .bind(id)
                        .execute(&db)
                        .await;
                        // Double-check suppress wasn't set during our write
                        if monitor.is_suppressed() {
                            continue;
                        }
                        let _ = app_handle.emit(
                            "clipboard-changed",
                            serde_json::json!({"action": "updated", "id": id}),
                        );
                    }
                    _ => {
                        let result = sqlx::query(
                            "INSERT INTO items (type, content, content_hash, file_path, preview, html_path, rtf_path) VALUES (?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(item_type)
                        .bind(&content)
                        .bind(&hash)
                        .bind(&file_path)
                        .bind(&preview)
                        .bind(&html_path)
                        .bind(&rtf_path)
                        .execute(&db)
                        .await;

                        if result.is_ok() {
                            // Double-check suppress wasn't set during our write
                            if monitor.is_suppressed() {
                                continue;
                            }
                            let _ = app_handle
                                .emit("clipboard-changed", serde_json::json!({"action": "new"}));
                        }
                    }
                }
            }
        });
    }
}

impl Clone for ClipboardMonitor {
    fn clone(&self) -> Self {
        Self {
            suppress_until: Arc::clone(&self.suppress_until),
            paused: Arc::clone(&self.paused),
            last_hash: Arc::clone(&self.last_hash),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- 组合哈希 ----------

    #[test]
    fn combo_hash_same_triple_is_stable() {
        let a = compute_text_combo_hash("hello", Some(b"<b>hello</b>"), Some(b"{\\rtf1}"));
        let b = compute_text_combo_hash("hello", Some(b"<b>hello</b>"), Some(b"{\\rtf1}"));
        assert_eq!(a, b, "同 (text,html,rtf) 三元组必须得到同一 hash");
    }

    #[test]
    fn combo_hash_differs_on_html() {
        let a = compute_text_combo_hash("hello", Some(b"<b>hello</b>"), None);
        let b = compute_text_combo_hash("hello", Some(b"<i>hello</i>"), None);
        assert_ne!(a, b, "同文字不同 html 不能被合并为同一条目");
    }

    #[test]
    fn combo_hash_differs_on_rtf() {
        let a = compute_text_combo_hash("hello", None, Some(b"{\\rtf1 a}"));
        let b = compute_text_combo_hash("hello", None, Some(b"{\\rtf1 b}"));
        assert_ne!(a, b);
    }

    #[test]
    fn combo_hash_no_formats_falls_back_to_text_hash() {
        let plain = compute_text_combo_hash("hello", None, None);
        assert_eq!(plain, compute_hash("hello"), "无 html/rtf 时必须与旧 hash 完全一致");
        // 有格式时必须与纯文本 hash 不同,否则旧条目会和新格式条目误合并
        assert_ne!(plain, compute_text_combo_hash("hello", Some(b"<b>x</b>"), None));
    }

    // ---------- 捕获归集(纯逻辑) ----------

    fn cap(text: &str, item_type: i32, html: Option<Vec<u8>>, rtf: Option<Vec<u8>>) -> CapturedContent {
        assemble_text_capture(text.to_string(), item_type, html, rtf, None)
    }

    #[test]
    fn capture_without_formats_keeps_legacy_shape() {
        let c = cap("普通文本", ClipboardType::Text as i32, None, None);
        assert_eq!(c.content, "普通文本");
        assert_eq!(c.item_type, ClipboardType::Text as i32);
        assert!(c.html_bytes.is_none());
        assert!(c.rtf_bytes.is_none());
        assert!(c.image_bytes.is_none());
        assert!(c.image_hash.is_none());
        assert_eq!(c.width, 0);
        assert_eq!(c.height, 0);
    }

    #[test]
    fn capture_with_formats_carries_bytes() {
        let c = cap(
            "图文",
            ClipboardType::Text as i32,
            Some(b"Version:0.9\r\n<html>".to_vec()),
            Some(b"{\\rtf1}".to_vec()),
        );
        assert_eq!(c.html_bytes.as_deref(), Some(&b"Version:0.9\r\n<html>"[..]));
        assert_eq!(c.rtf_bytes.as_deref(), Some(&b"{\\rtf1}"[..]));
        // content 仍是纯文本(排除规则/预览/前端展示的判定基准不变)
        assert_eq!(c.content, "图文");
    }

    #[test]
    fn html_only_capture_keeps_rtf_none() {
        let c = cap("t", ClipboardType::Link as i32, Some(b"<a/>".to_vec()), None);
        assert!(c.html_bytes.is_some());
        assert!(c.rtf_bytes.is_none());
    }

    // ---------- 落盘文件名 ----------

    #[test]
    fn format_file_name_uses_hash_prefix_and_ext() {
        // 固定 hash 的字面量期望(不用同一表达式复算,避免断言恒真)
        let hash = "0123456789abcdef0123456789abcdef";
        assert_eq!(format!("{}.{}", &hash[..16], "html"), "0123456789abcdef.html");
        assert_eq!(format!("{}.{}", &hash[..16], "rtf"), "0123456789abcdef.rtf");
        // 短 hash 不越界
        let short = "abc";
        assert_eq!(format!("{}.{}", &short[..short.len().min(16)], "rtf"), "abc.rtf");
    }

    // ---------- 捕获侧真机往返(读侧覆盖) ----------

    /// 写入三格式 → 调捕获读取路径 → 断言读回的 html/rtf 字节逐字节一致、
    /// 文本一致。这是对 read_text_with_extended_formats 持 guard 读取的
    /// 直接验证(不持 guard 的 GetClipboardData 必然失败,读回 None 会在此暴露)。
    /// 真机剪贴板测试,须单线程运行(多测试并行会抢占全局剪贴板):
    /// cargo test --lib --release -- --ignored --test-threads=1 services::clipboard_monitor
    #[test]
    #[ignore = "需要交互式桌面会话的真机剪贴板,CI/无头环境不可靠"]
    fn capture_roundtrip_reads_all_formats() {
        let text = "捕获往返 roundtrip";
        let html: &[u8] = b"Version:0.9\r\nStartHTML:0000000105\r\n<html><body>rich</body></html>";
        let rtf: &[u8] = b"{\\rtf1\\ansi rich}";

        // 用写回侧的同一套函数写入(与恢复端行为一致)
        crate::services::paste_service::write_rich_text_to_clipboard(
            text, Some(html), Some(rtf),
        )
        .expect("写入三格式应成功");

        // 捕获读取:必须读回全部三种
        let captured = read_text_with_extended_formats()
            .expect("捕获读取必须成功(剪贴板上有 CF_UNICODETEXT)");
        assert_eq!(captured.text, text, "文本必须往返一致");
        assert_eq!(
            captured.html.as_deref(),
            Some(html),
            "HTML Format 字节必须逐字节往返(含 CF_HTML 头)"
        );
        assert_eq!(
            captured.rtf.as_deref(),
            Some(rtf),
            "Rich Text Format 字节必须逐字节往返"
        );
    }

    /// 纯文本剪贴板(无扩展格式)上捕获:html/rtf 必须为 None,文本正常读回。
    /// 锁定"扩展格式缺失时行为与旧路径一致"的真机侧验证。
    #[test]
    #[ignore = "需要真机剪贴板"]
    fn capture_plain_text_reads_no_formats() {
        crate::services::paste_service::write_rich_text_to_clipboard("纯文本捕获", None, None)
            .expect("写纯文本应成功");
        let captured = read_text_with_extended_formats()
            .expect("纯文本剪贴板上捕获读取应成功");
        assert_eq!(captured.text, "纯文本捕获");
        assert!(captured.html.is_none(), "无 HTML Format 时必须为 None");
        assert!(captured.rtf.is_none(), "无 RTF 时必须为 None");
    }
}
