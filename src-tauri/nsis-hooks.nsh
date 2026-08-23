; NSIS 安装钩子:覆盖安装/卸载时保全用户数据(config 与 data)。
; 备份放在安装目录的同级 clipboard-userdata(必须在 $INSTDIR 之外,
; 否则卸载器的 RMDir /r $INSTDIR 会连备份一起删除)。
; 流程:卸载/安装前移出 → 新文件复制完毕后移回(并存则合并)。

!define USERDATA_BACKUP "$INSTDIR\..\clipboard-userdata"

; 覆盖安装/卸载前结束正在运行的应用(含 WebView2 子进程)。
; 应用以管理员运行(manifest requireAdministrator),普通权限 taskkill 杀不掉
; 提权主进程。用 taskkill 退出码判断:0=已结束 128=未运行
; 其他(如 255)=进程存在但权限不足,提权重杀(弹一次 UAC)。
; 注意:不能用 FindProcessCurrentUser 检测——它看不到提权进程(误报未运行)
!macro KILL_RUNNING_APP
  kill_app_retry:
    nsExec::ExecToStack 'taskkill /F /T /IM "clipboard-manager-tauri.exe"'
    Pop $0
    Pop $9
    Sleep 500
    ${If} $0 <> 0
    ${AndIf} $0 <> 128
      ; 提权重杀(弹一次 UAC);逗号分隔参数避免 NSIS 嵌套引号问题
      nsExec::Exec '$SYSDIR\WindowsPowerShell\v1.0\powershell.exe -NoProfile -WindowStyle Hidden -Command "Start-Process taskkill -ArgumentList /F,/T,/IM,clipboard-manager-tauri.exe -Verb RunAs -Wait"'
      Pop $9
      Sleep 800
      nsExec::ExecToStack 'tasklist /FI "IMAGENAME eq clipboard-manager-tauri.exe"'
      Pop $0
      Pop $9
      ${If} $0 = 0
        MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "剪贴板管理器正在运行且无法自动关闭(以管理员身份运行)。$\n$\n请从系统托盘图标右键退出应用,或在任务管理器中结束 clipboard-manager-tauri.exe,然后点击「重试」。$\n$\n点击「取消」将中止当前操作。" IDRETRY kill_app_retry
        Abort "无法关闭正在运行的剪贴板管理器"
      ${EndIf}
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
