//! 临时工具:用 ApplicationActivationManager 以稀疏包 AUMID 激活 identity_probe,
//! 验证「Shell 激活启动的进程才获得包身份 → NotificationChanged 订阅成功」。
//! 激活后探针的输出写在 target/debug/identity_probe.log。

use windows::core::HSTRING;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_LOCAL_SERVER};
use windows::Win32::UI::Shell::{
    ApplicationActivationManager, IApplicationActivationManager, ACTIVATEOPTIONS,
};

const AUMID: &str = "ClipboardManagerIdentity_z0fnhbkv2vcxr!SmsHelper";

fn main() {
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let mgr: IApplicationActivationManager = CoCreateInstance(
            &ApplicationActivationManager,
            None,
            CLSCTX_LOCAL_SERVER,
        )
        .expect("CoCreateInstance(ApplicationActivationManager) 失败");

        match mgr.ActivateApplication(
            &HSTRING::from(AUMID),
            &HSTRING::from(""),
            ACTIVATEOPTIONS(0),
        ) {
            Ok(pid) => println!("activated, pid = {pid}"),
            Err(e) => {
                eprintln!("ActivateApplication 失败: {e} (HRESULT {:#010X})", e.code().0 as u32);
                std::process::exit(1);
            }
        }
    }
}
