; NSIS 安装钩子:覆盖安装/卸载时保全用户数据(config 与 data)。
; 备份放在安装目录的同级 sparrow-userdata(必须在 $INSTDIR 之外,
; 否则卸载器的 RMDir /r $INSTDIR 会连备份一起删除)。
; 流程:卸载/安装前移出 → 新文件复制完毕后移回(并存则合并)。
;
; 2.0.0 改名(Sparrow / 剪雀)相关的兼容处理:
; - 1.x(Clipboard / clipboard-manager-tauri)的安装目录**由用户在向导里选**,
;   常见是 %LOCALAPPDATA%\clipboard,也可能是 D:\clipboard 这类自定义路径。
;   所以旧目录不写死:installer.nsi 的 DetectLegacyInstallDir 读旧卸载项的
;   InstallLocation 写进 $LegacyInstallDir,这里优先用它。
; - 检测到旧版就**原地升级**:$INSTDIR 已被设为旧目录,旧 config/data 原地
;   保留(只走上面的备份-还原),不再另建目录、也不再复制数据。
; - 只有用户手动把安装路径改到别处时,才把旧 config/data 递归复制过去,
;   且不覆盖新目录已有文件,失败只记日志不 Abort。
; - 安装完成后清理 1.x 的卸载项、设置键、快捷方式与旧主程序,让「应用与功能」
;   里不再留旧条目;旧目录里其它文件不动,不自动运行旧卸载器(避免静默卸载
;   把用户数据一起带走)。
; - 旧版自启动计划任务 ClipboardManagerAutostart 一并删除,避免新旧两个任务
;   同时开机拉起(新任务 SparrowAutostart 由应用启动时自行重建)。
!define USERDATA_BACKUP "$INSTDIR\..\sparrow-userdata"
; 1.x 的默认安装目录:仅在注册表里查不到旧安装位置时兜底使用(不写入、不删除)
!define LEGACY_INSTDIR "$LOCALAPPDATA\clipboard"
; 1.x 的卸载项注册表键。1.x 的 PRODUCTNAME 是 clipboard,所以键名固定是 clipboard
!define LEGACY_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\clipboard"

; 1.x 安装目录(installer.nsi 的 DetectLegacyInstallDir 填写;空串=未检测到)
Var LegacyInstallDir
; 数据接管时的源目录(检测结果,或退回 LEGACY_INSTDIR)
Var LegacyDataDir
; nsExec 结果暂存(xcopy / schtasks 的退出码与输出)
Var LegacyExecRet
Var LegacyExecOut

; 旧版进程:best-effort 结束。只按精确映像名杀,不做通配,避免误杀同名无关
; 进程;杀不掉不阻断安装 —— 原地升级时旧主程序就躺在 $INSTDIR 里,不杀它
; 后面无法覆盖 sparrow.exe,所以这里仍然尽力而为。
!macro KILL_LEGACY_APP
  nsExec::ExecToStack 'taskkill /F /IM "clipboard-manager-tauri.exe"'
  Pop $0
  Pop $9
!macroend

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
  ; 1.x 旧进程(clipboard-manager-tauri.exe)顺带结束:原地升级时它就在
  ; $INSTDIR 里,而且可能仍持有 data\clipboard.db
  !insertmacro KILL_LEGACY_APP
  kill_main_retry:
    nsExec::ExecToStack 'taskkill /F /IM "sparrow.exe"'
    Pop $0
    Pop $9
    Sleep 500
    ${If} $0 <> 0
    ${AndIf} $0 <> 128
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "剪雀正在运行但无法自动结束。$\n$\n请从系统托盘图标右键退出应用,或在任务管理器中结束 sparrow.exe(若结束不掉说明进程已僵死,重启电脑后再试),然后点击「重试」。$\n$\n点击「取消」将中止当前操作。" IDRETRY kill_main_retry
      Abort "无法关闭正在运行的剪雀"
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

; 旧目录数据接管(1.x → 2.0.0)。
; 源目录:优先 $LegacyInstallDir(注册表读到的旧安装位置),没有才用默认目录。
; 规则:1) 原地升级($INSTDIR 就是旧目录)直接跳过 —— 旧数据本来就在当前
;       目录,备份-还原流程已经把它带回来了,再复制等于自我覆盖;
;       2) 只在新目录还没有对应目录/文件时复制,绝不覆盖新目录已有内容;
;       3) 任何失败都只记日志继续,不 Abort;4) 旧目录原样保留。
!macro MIGRATE_LEGACY_USERDATA
  StrCpy $LegacyDataDir $LegacyInstallDir
  ${If} $LegacyDataDir == ""
    StrCpy $LegacyDataDir "${LEGACY_INSTDIR}"
  ${EndIf}

  ${If} "$INSTDIR" == "$LegacyDataDir"
    ; 原地升级:无需复制
    Goto mig_legacy_done
  ${EndIf}
  IfFileExists "$LegacyDataDir\*.*" 0 mig_legacy_done

  IfFileExists "$INSTDIR\config\*.*" mig_legacy_skip_config
    IfFileExists "$LegacyDataDir\config\*.*" 0 mig_legacy_skip_config
      DetailPrint "迁移旧版配置: $LegacyDataDir\config -> $INSTDIR\config"
      nsExec::ExecToStack 'xcopy /E /I /Y /Q "$LegacyDataDir\config" "$INSTDIR\config"'
      Pop $LegacyExecRet
      Pop $LegacyExecOut
      ${If} $LegacyExecRet <> 0
        DetailPrint "旧版配置迁移失败(exit $LegacyExecRet),已跳过;旧目录保留,可手动复制"
      ${EndIf}
  mig_legacy_skip_config:

  IfFileExists "$INSTDIR\data\*.*" mig_legacy_skip_data
    IfFileExists "$LegacyDataDir\data\*.*" 0 mig_legacy_skip_data
      DetailPrint "迁移旧版数据: $LegacyDataDir\data -> $INSTDIR\data"
      nsExec::ExecToStack 'xcopy /E /I /Y /Q "$LegacyDataDir\data" "$INSTDIR\data"'
      Pop $LegacyExecRet
      Pop $LegacyExecOut
      ${If} $LegacyExecRet <> 0
        DetailPrint "旧版数据迁移失败(exit $LegacyExecRet),已跳过;旧目录保留,可手动复制"
      ${EndIf}
  mig_legacy_skip_data:
  mig_legacy_done:
!macroend

; 1.x 残留清理:卸载项、设置键、快捷方式、旧主程序、旧自启动任务。
; 只删下面这些**精确名字**,不用通配;全部 best-effort —— 删不掉只影响观感,
; 绝不 Abort 安装。原地升级时这些清理尤其重要:否则「应用与功能」里会同时
; 留着 clipboard 与 Sparrow 两个条目。
!macro CLEANUP_LEGACY_INSTALL
  ; 旧卸载项(SHCTX 在 currentUser 安装器里就是 HKCU,HKLM 只是兜底)
  DeleteRegKey SHCTX "${LEGACY_UNINSTKEY}"
  DeleteRegKey HKCU "${LEGACY_UNINSTKEY}"
  DeleteRegKey HKLM "${LEGACY_UNINSTKEY}"
  ; 1.x 的应用设置键(自定义存储位置指针等)
  DeleteRegKey HKCU "Software\lyz\clipboard"
  DeleteRegKey HKLM "Software\lyz\clipboard"

  ; 1.x 快捷方式(开始菜单两种布局 + 桌面两种命名)
  Delete "$SMPROGRAMS\clipboard.lnk"
  Delete "$SMPROGRAMS\clipboard\clipboard.lnk"
  RMDir "$SMPROGRAMS\clipboard" ; 仅当目录已空时才会被删掉
  Delete "$DESKTOP\clipboard.lnk"
  Delete "$DESKTOP\clipboard-manager-tauri.lnk"

  ; 旧主程序:原地升级后它就躺在 $INSTDIR 里,删掉避免两个 exe 并存
  Delete "$INSTDIR\clipboard-manager-tauri.exe"

  ; 双装场景(旧版在 $LegacyInstallDir、新版在 $INSTDIR,两者不同):旧目录里
  ; 会留下没有卸载项的 clipboard-manager-tauri.exe / uninstall.exe 当孤儿。
  ; 这里清理旧程序本体,数据保留给用户确认 —— 只删这两个精确文件名,
  ; 不动旧目录里的 config/data,也不删旧目录本身。
  ${If} $LegacyInstallDir != ""
  ${AndIf} "$LegacyInstallDir" != "$INSTDIR"
    Delete "$LegacyInstallDir\clipboard-manager-tauri.exe"
    Delete "$LegacyInstallDir\uninstall.exe"
  ${EndIf}

  ; 1.x 的自启动计划任务:best-effort 删除,忽略失败(新任务由应用启动时重建)
  nsExec::ExecToStack 'schtasks /Delete /TN "ClipboardManagerAutostart" /F'
  Pop $LegacyExecRet
  Pop $LegacyExecOut
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
; 不携带这两个目录)则把备份内容合并进去。随后做旧目录接管与 1.x 残留清理。
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

  ; 1.x 旧安装目录的数据接管(原地升级时自动跳过)+ 1.x 残留清理
  !insertmacro MIGRATE_LEGACY_USERDATA
  !insertmacro CLEANUP_LEGACY_INSTALL
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
