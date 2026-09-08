use std::path::PathBuf;
use std::process::Command;

fn main() {
    // 主程序需要 requireAdministrator 清单,但不要把清单交给 tauri_build 注入:
    // tauri_build 的清单会随 lib 静态库带进同包的所有 bin(含 identity_probe 探针),
    // 使探针也必须以管理员身份启动。这里改为只对主程序二进制单独嵌入该清单
    // (见下方 link-arg-bin),探针保持 rustc 默认 asInvoker,普通用户即可运行。
    tauri_build::try_build(
        tauri_build::Attributes::new()
            // 不注入默认清单:默认清单会作为全局资源带进同包所有 bin(含 identity_probe
            // 探针),使探针也必须管理员启动。这里用 new_without_app_manifest 关闭默认
            // 清单,改由下方 embed_main_manifest 仅对主程序 bin 单独链接 requireAdministrator
            // 清单,探针保持 rustc 默认 asInvoker,普通用户即可运行。
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
    )
    .expect("failed to run tauri-build");

    embed_main_manifest();
}

/// 把 app.manifest 编译成 .res 并链接到主程序二进制 clipboard-manager-tauri。
/// 用 .res 链接可覆盖 rustc 默认清单(不会与探针的默认清单冲突),且清单只进
/// 主程序,不进 lib,因此同包的 identity_probe 探针不受影响。
fn embed_main_manifest() {
    let manifest = format!("{}\\app.manifest", env!("CARGO_MANIFEST_DIR"));
    println!("cargo:rerun-if-changed={}", manifest);

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let rc_path = PathBuf::from(&out_dir).join("app_manifest.rc");
    // 手动定义 RT_MANIFEST(24),避免 #include <winuser.h> 带来的额外依赖;
    // 路径用反斜杠(rc.exe 对清单外部文件要求 2.03 资源格式,正反斜杠敏感)。
    let rc = format!(
        "#define RT_MANIFEST 24\n1 RT_MANIFEST \"{}\"\n",
        manifest.replace('\\', "\\\\")
    );
    std::fs::write(&rc_path, rc).expect("failed to write app_manifest.rc");

    let rc_exe = find_rc().expect(
        "找不到 rc.exe(Windows SDK 的 rc.exe)。请确认已安装 Windows SDK 且其 bin 目录在 PATH 中。",
    );
    let res_path = PathBuf::from(&out_dir).join("app_manifest.res");
    let status = Command::new(&rc_exe)
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .status()
        .expect("failed to run rc.exe");
    assert!(status.success(), "rc.exe 编译清单失败");

    // 仅作用于主程序二进制,不影响同包探针
    println!(
        "cargo:rustc-link-arg-bin=clipboard-manager-tauri={}",
        res_path.display()
    );
}

/// 在 Windows SDK 各版本 bin 目录下查找 rc.exe(x64)。
fn find_rc() -> Option<PathBuf> {
    let base = "C:\\Program Files (x86)\\Windows Kits\\10\\bin";
    let Ok(entries) = std::fs::read_dir(base) else {
        return None;
    };
    for entry in entries.flatten() {
        let candidate = entry.path().join("x64").join("rc.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
