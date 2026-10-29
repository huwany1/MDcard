# SPDX-License-Identifier: AGPL-3.0-only
import json,shutil,subprocess
from datetime import datetime,timezone
from pathlib import Path
root=Path(__file__).resolve().parent.parent;e=root/'evidence/release-015'
target=root/'docs/releases/0.1.5-ACCEPTANCE.md'
if target.exists() and not (e/'acceptance-working-draft.md').exists():shutil.copy2(target,e/'acceptance-working-draft.md')
guest=e/'windows-clean-guest/guest-evidence/result.json'
guest_ok=guest.exists() and json.loads(guest.read_text(encoding='utf-8-sig')).get('passed') is True
status=('Windows 无网干净来宾已通过；两平台本地交付验收已完成，归档校验由发行清单及 SHA256SUMS 记录。最终验收记录时间（UTC）：'+datetime.now(timezone.utc).isoformat()) if guest_ok else '收尾中：Windows 全新操作系统来宾仍在实际安装/验收；未将 C4 标为全部通过。其余已完成证据如下，本报告不冒充整个目标已完成。'
text=r'''# MDcard 0.1.5 本地交付验收

STATUS

## 选定产物和源码身份

| 项目 | 冻结身份 |
| --- | --- |
| 任务开始与交付前复核的 `D:\MDcard` 主线 | `3fc25d54f7c5964f2a1a7445737d54621f3a12c8`，干净工作区 |
| 两平台原生应用及完整资源的 clean build 源码 | `f2bebfb46278b99e880f31383fd5afe5c23fa9bb` |
| 最终 Windows 安装器打包源码 | `7a429572108bcc3de9e0987fceef8171e5c55e1f` |
| 最终 Linux 控制脚本打包源码 | `2a883e9`；至 `7a42957` 的 Linux/产品输入无差异 |
| Windows setup SHA256 / 大小 | `ab961fb64ac6cfdd2f4c782d16b81405fc70d69d661f090863f0e31be502199d` / 276,212,458 B |
| Linux deb SHA256 / 大小 | `69418eadbff59b379ad853eb6068ce15cc05f4a945e4f1f3e5a259c7404f0e02` / 283,374,264 B |
| 已安装 Windows MDcard.exe | `933f1591a0fd47806e9e2eb2eb29ba2489493236e836ca8a66f83a7240b4c127` |
| 已安装 Linux mdcard（staging strip 后） | `55d033a818c826cd0f4b7b0b7b94c9e23b57dd6615b00456a73d132bb97ddf49` |
| Windows / Linux payload manifest SHA256 | `d495d509e73ff2f0cb1ba4892172cc6ca5fae1ec2817ca333e739b0bf36ba513` / `102435a5e9a761bc97069f485cfd03c1d2b0a1b1859fcb8664a1ffe83628eee1` |

`eeefffd08addac41bd962e6080b8932fe011469a` 是版本统一提交；其后产品业务、字体、renderer 自有源码未改动。后续改动为构建/打包、恢复保护及 QA/报告。Linux clean build 原始未 strip ELF 为 `24a239…`，不能把它写成已安装 SHA。完整提交、最终源码 ZIP / Git bundle、QA/报告快照和工具输入由 `MDcard-0.1.5-release.json` 绑定。没有复用来源不明的旧二进制，也没有修改既有 0.1.0 发行记录。应用、安装器、deb 和清单版本均为 0.1.5。

## 八类标准矩阵

| 标准 | 实际结果与主要证据 | 边界 |
| --- | --- | --- |
| C1 身份 | 独立 TEMP clone 完整 build 脚本 exit0、两平台同源，最终打包输入冻结；`clean-source-rebuild-final-r2.log`、`final-clean-build-identity.json`、最终逐文件 manifest | 源码归档/发行清单额外记录后续 QA 文档快照，不将它冒充原生重编译 |
| C2 Windows 安装 | 用户级，无提权；中文空格路径、开始菜单、可选桌面/Open With、不更改 Typora UserChoice；真实两页 Next、自动启动933f；`final-windows-wizard-r{2,3,4}`、`final-windows-protected-install` | 自动化探查 caption 后恢复同一向导，bootstrap 退出码不可取，完整 success/Run 日志与自动创建进程独立证明成功；实际最后按钮是 Next |
| C3 Linux 安装 | Debian12 amd64 dpkg/apt 标准包、非 root 运行、桌面/图标/命令入口、权限无 group/world writable；最终三次fresh、重装/升级/移除/purge；`final-linux-protected-install`、`linux-different-upgrade-protected` | 仅实测 Debian12/X11/Openbox/Xvfb/软件 Mesa，不扩展声称 Ubuntu/Wayland/物理 GPU 已验收 |
| C4 完整运行条件 | 所需 Node/Java/Chromium/Pandoc/必要字体/许可证随包；Linux 无源码挂载、无系统开发运行时、PATH空、Docker禁网；Windows 全新 LTSC 来宾实际安装 AB/933f，无NIC、无开发工具、非管理员令牌、4引擎9导出和键盘编辑保存通过 | `windows-clean-guest/guest-evidence/result.json`；来宾不替代宿主 final933 的完整 New/Open/SaveAs/IME/恢复验收，宿主清PATH也不等同干净OS |
| C5 可复核成本 | 同payload ZIP6/gzip6、至少三次原始样本、实际默认Open及 A/B；新保护包重新测三次安装；`final-cost-model.json` 与 `0.1.5-COSTS.md` | 下载是固定带宽估算；分阶段配对合成，不伪装连续人工实测；快网 Windows 可能更慢 |
| C6 安装后功能 | 两平台编辑/保存/SaveAs/搜索/Split/四引擎/九格式；真实 Pinyin/IBus 预编辑与“你好”落盘；实际自动保存/强制终止恢复；完整清单/视觉/18导出结构独立复核 | 最终Windows933f autosave/Core恢复证据见 `final-windows-autosave` / `final-windows-recovery`；早期131D只作历史支持，不混写SHA |
| C7 数据与中断 | 运行中升级/卸载拒绝且不杀进程；完整旧版备份先SHA验证；真实Win复制期取消5、1847旧文件全恢复；Linux不同内容SIGINT保留完整旧版且手工恢复零差；用户资料保留 | Windows备份约980MB/30s；Linux SIGINT非自动完整回滚，已实测明确手动恢复+标准repair，不承诺任意断电/磁盘损坏自动事务 |
| C8 可维护交付 | 固定输入、脚本入库、完整clean build与最终实际打包、SHA、源码快照、指南/说明/成本和证据归档 | 最终目录装订后以SHA256SUMS和发行清单为准；仅本地交付，未推送或公开发布 |

## 最终已安装功能证据

- Windows：`final-windows-gui-r3/measurements.json` 的3+3次默认无文档参数启动、原生Open、保存、搜索、Split与菜单PDF；`final-windows-default/result.json` 的 New/Open/SaveAs、默认用户目录、dirty安装7/卸载1；`final-windows-ime/result.json` 真实 Microsoft Pinyin。
- Linux：`final-linux-gui/{measurements,lifecycle}.json` 默认Open、SaveAs、自动保存、真实强制终止后恢复为未保存副本；`final-linux-engines/result.json` 四引擎+九导出，网络none；`installed-linux-ime/result.json` 真实 IBus/libpinyin。Linux已安装原生55D与最终f2字节相同，最终新deb只变控制部分，全部opt文件有完整SHA验证。
- 九种格式：HTML、PDF、PNG、JPEG、DOCX、EPUB、Marp HTML、Marp PDF、Marp PPTX。不是仅凭文件存在：校验引擎返回SHA、格式魔数/ZIP CRC、关键容器成员；中文/数学/表格/三类图表PNG经主代理独立目视。Windows最新实际运行条件的九导出由干净来宾补充。
- Node/Java/Chromium/Pandoc均从实际安装位置调用，无运行时下载或开发工具拼装。Linux初始运行环境包清单及依赖方案保留在 `linux-base-packages.txt` / `linux-install-plan.txt`；该基底Depends额外下载0，其他机器按apt另计。

## 真实失败、修复与恢复

原生 Inno 在覆盖后取消不会恢复全部旧文件：第一次复制期取消后1846旧文件中5项已变。原生 dpkg 不同内容升级被 SIGINT 打断也留下新 README。两项失败都原样保留，没有拿同字节重装/前置取消替代。由此增加最小打包恢复保护，未改业务保存/恢复语义。

Windows最终AB：先逐文件复制并验证旧目录、应用身份及入口；备份不完整/残留目录均拒绝覆盖，独立测试含旧exe已缺失分支。真实进度Cancel是在旧7364896已完全变为933f后触发；结果 `windows-final-protected-copy-cancel/result.json` 为exit5、1847旧文件零差、PE回0.1.0，取消到恢复退出28.87秒。完整直接恢复还验证了注册/桌面入口与安装期间新建的未知用户文档不被删除。

Linux最终694：preinst完整旧目录SHA备份，postinst对全部新文件SHA通过后清理备份。有效全旧payload QA夹具保留相同可运行0.1.5 exe/运行时，仅README/manifest字节与dpkg低版本元数据不同；不是历史0.1.4正式包。观察真实旧README被新字节替换后SIGINT，自动未完全恢复；明确命令 `sudo sh /var/lib/mdcard/installer-backup/restore.sh --mdcard-restore` 成功恢复全部旧文件，再 `dpkg --configure -a` 与本地包重装成功。完整证据含 `automaticRestore=false`、`manualPayloadMismatches=[]`，不宣称自动回滚。

临时QA桌面入口曾覆盖便携入口时均先备份并恢复，Typora默认关联未变，未替换 `D:\MDcard\App`，不会默认删除文档或用户配置/恢复目录。最终QA清理与用户资料哈希见发行证据。

## 验收设备、保留限制及失败轮次

Windows宿主有开发工具，清环境与TEMP测试保留该边界；无网官方eval来宾独立验证此前不能覆盖的干净OS条件。来宾TCG的硬件检查绕过、额外ISO/QEMU/IBus下载均仅QA成本，不伪造普通机器性能或产品依赖。Windows安装器本次未代码签名，SmartScreen额外确认未做浏览器下载实测。

全新来宾实际为 Windows 11 Enterprise LTSC 2024 x64，OS 10.0.26100.0、PowerShell 5.1.26100.1591，标准用户令牌；安装阶段用TCG，正式功能用经实际执行探针确认的WHPX，4GB/2vCPU，无虚拟网卡。来宾 node/npm/java/pandoc/rustc/cargo/gcc/git 均不存在于系统 PATH，也未挂载项目目录。实际已安装 AB/933f 的四引擎和九导出13项全通过；真实键盘把 `Initial.` 改成46B的两行内容，Ctrl+S后读取磁盘SHA为 `aa326067fc4ef6a72d0e830cab149ef2adca0d5f6764b1b7b1a3cf0c88b8f7e2`，UI dirty=false。原始ZIP经CRC验证回传，内容与 UI JSON 保留在 `windows-clean-guest/guest-evidence`。

来宾复杂 New/SaveAs helper 的英文选择器、PS5 UTF-8 BOM、早期媒体文本重写语法错误以及指针焦点失败均保留为QA失败，未将其写为通过。最终使用可见键盘模式切换与保存闭环；完整 New/Open/SaveAs 由宿主实际安装的同一933f二进制验收覆盖。屏幕中的系统评估许可状态只记录，未更改时钟或绕过激活。来宾原始验收完成UTC为2026-10-29T15:22:36.2061398Z，即日本当地2026-10-30 00:22。

Windows旧前景/输入caption失败、v4成功慢首轮（B追加34s）、Linux旧Preference几何漂移、IBus发现失败、Windows helper模块搜索路径污染、同版与不同内容中断探查都保留。最终采样固定新包/同payload/无重负载，未因首轮慢而静默丢弃成功结果。详见 `0.1.5-COSTS.md` 与原始证据。

Rust clean build最终为357 passed / 0 failed / 6 ignored。ignored分别是内部subprocess actor、显式独立tmpfs/非root的disk-full/permission-denied、CPU竞争取消时序探针、完整搜索竞争计时、432布局矩阵。它们不被声称通过；实际安装后的正常保存/恢复与打包中断保护是独立验收。没有将编译成功或Windows通过当Linux验收，也没有承诺所有OS/驱动/输入法和任意电源故障。

最终用户少下载多少、少操作几步、A/B省多久及代价，见 `0.1.5-COSTS.md`；其中同时展示正收益与快网回退，未把更小等同更快。
'''
linux_commit=subprocess.check_output(['git','rev-parse','2a883e9'],cwd=root,text=True).strip()
target.write_text(text.replace('STATUS',status).replace('`2a883e9`；',f'`{linux_commit}`；').replace(r'\u005c','\\'),encoding='utf-8',newline='\n')
print('Updated acceptance; clean guest passed='+str(guest_ok))
