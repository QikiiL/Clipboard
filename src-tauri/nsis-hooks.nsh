; NSIS 安装钩子:覆盖安装/卸载时保全用户数据(config 与 data)。
; 备份放在安装目录的同级 clipboard-userdata(必须在 $INSTDIR 之外,
; 否则卸载器的 RMDir /r $INSTDIR 会连备份一起删除)。
; 流程:卸载/安装前移出 → 新文件复制完毕后移回(并存则合并)。

!define USERDATA_BACKUP "$INSTDIR\..\clipboard-userdata"

; 覆盖安装/卸载前结束正在运行的应用与短信助手。安装器/卸载器均为管理员权限
; (RequestExecutionLevel admin),taskkill 可直接结束提权应用。
; 要点:1) 退出码判断 0=已结束 128=未运行 其他=无法结束
; 2) 不用 /T 树杀:先杀子进程会把主进程卡成僵尸,必须先杀主进程
; 3) 顺序:先杀主程序,再杀 sms-helper。helper 由 Shell 以 AUMID 激活,
;    不是主程序的子进程(/T 也杀不到),主程序死后它不会立刻消失 ——
;    带 --parent 看门狗的也要 ≤5s 才自灭,老版本/无参数的可驻留 60 分钟,
;    不显式杀掉就会锁住安装目录里的 sms-helper.exe,覆盖安装报
;    「无法打开要写入的文件」(v1.0.0 升级实测)。先杀主程序是因为
;    监工循环在主程序里,防止 helper 被杀后 30 秒内被重新拉起
!macro KILL_RUNNING_APP
  kill_main_retry:
    nsExec::ExecToStack 'taskkill /F /IM "clipboard-manager-tauri.exe"'
    Pop $0
    Pop $9
    Sleep 500
    ${If} $0 <> 0
    ${AndIf} $0 <> 128
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "剪贴板管理器正在运行但无法自动结束。$\n$\n请从系统托盘图标右键退出应用,或在任务管理器中结束 clipboard-manager-tauri.exe(若结束不掉说明进程已僵死,重启电脑后再试),然后点击「重试」。$\n$\n点击「取消」将中止当前操作。" IDRETRY kill_main_retry
      Abort "无法关闭正在运行的剪贴板管理器"
    ${EndIf}
  kill_helper_retry:
    nsExec::ExecToStack 'taskkill /F /IM "sms-helper.exe"'
    Pop $0
    Pop $9
    Sleep 300
    ${If} $0 <> 0
    ${AndIf} $0 <> 128
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "短信助手进程(sms-helper.exe)无法自动结束。$\n$\n请在任务管理器中手动结束 sms-helper.exe,然后点击「重试」。$\n$\n点击「取消」将中止当前操作。" IDRETRY kill_helper_retry
      Abort "无法关闭正在运行的短信助手进程"
    ${EndIf}
!macroend

; 覆盖安装:新文件复制前,若目录还在(未被旧卸载器处理),移出到备份
!macro NSIS_HOOK_PREINSTALL
  !insertmacro KILL_RUNNING_APP
  IfFileExists "$INSTDIR\data\*.*" 0 +3
    CreateDirectory "${USERDATA_BACKUP}"
    Rename "$INSTDIR\data" "${USERDATA_BACKUP}\data"
  IfFileExists "$INSTDIR\config\*.*" 0 +3
    CreateDirectory "${USERDATA_BACKUP}"
    Rename "$INSTDIR\config" "${USERDATA_BACKUP}\config"
!macroend

; 新文件复制完毕:目标目录不存在则整体改回原名;并存(理论上安装包
; 不携带这两个目录)则把备份内容合并进去
!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "${USERDATA_BACKUP}\data\*.*" 0 ud_nodata
    IfFileExists "$INSTDIR\data\*.*" 0 ud_restore_data
      CopyFiles /SILENT "${USERDATA_BACKUP}\data\*.*" "$INSTDIR\data"
      RMDir /r "${USERDATA_BACKUP}\data"
      Goto ud_nodata
  ud_restore_data:
    Rename "${USERDATA_BACKUP}\data" "$INSTDIR\data"
  ud_nodata:

  IfFileExists "${USERDATA_BACKUP}\config\*.*" 0 ud_noconfig
    IfFileExists "$INSTDIR\config\*.*" 0 ud_restore_config
      CopyFiles /SILENT "${USERDATA_BACKUP}\config\*.*" "$INSTDIR\config"
      RMDir /r "${USERDATA_BACKUP}\config"
      Goto ud_noconfig
  ud_restore_config:
    Rename "${USERDATA_BACKUP}\config" "$INSTDIR\config"
  ud_noconfig:

  ; 合并完成且备份已空时移除壳目录;非空则保留(数据兜底)
  RMDir "${USERDATA_BACKUP}"
!macroend

; 卸载(含覆盖安装触发的静默卸载)前:结束运行中的应用,再把用户数据移到安装目录之外
!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro KILL_RUNNING_APP
  IfFileExists "$INSTDIR\data\*.*" 0 +3
    CreateDirectory "${USERDATA_BACKUP}"
    Rename "$INSTDIR\data" "${USERDATA_BACKUP}\data"
  IfFileExists "$INSTDIR\config\*.*" 0 +3
    CreateDirectory "${USERDATA_BACKUP}"
    Rename "$INSTDIR\config" "${USERDATA_BACKUP}\config"
!macroend

; 纯卸载后备份保留在同级目录,数据不丢,由用户自行处置
!macro NSIS_HOOK_POSTUNINSTALL
!macroend
