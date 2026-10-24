# MDcard UI 最终候选验收（2026-10-22）

结论：本轮候选在下列本机与已列测试范围内验收通过，未发现阻断编辑、保存或现有关键操作的问题。性能中的失败门槛单独列出，未验证项不计为通过。实现记录与冻结契约见 [UI 调整与美化](UI_POLISH_2026-10-22.md)。

主代理 最终独立验收汇总（历史本地证据：`../evidence/ui-polish-2026-10-22/master/acceptance-r11.json`，未公开） 记录15项独立检查与九项标准、失败子项和未验证环境。Windows/Linux候选相关测试进程无遗留。

## 候选身份与运行入口

产品源码提交：`c430a7d0a9c6ad44eb70a13a2e59fa75b2c2dc4b`，分支 `codex/ui-polish-2026-10-22`；buildId：`ui-polish-r11-20261022`。后续验收文档提交与这个产品提交分别记录，候选包保持此身份。

| 项目 | Windows x64 | Linux x64 |
| --- | --- | --- |
| 程序 | MDcard.exe（历史本地证据：`../artifacts/ui-polish-2026-10-22/packages/ui-polish-r11-20261022/windows/MDcard.exe`，未公开） | mdcard（历史本地证据：`../artifacts/ui-polish-2026-10-22/packages/ui-polish-r11-20261022/linux/mdcard`，未公开） |
| 完整包 | ZIP（历史本地证据：`../artifacts/ui-polish-2026-10-22/packages/ui-polish-r11-20261022/MDcard-0.1.1-ui-polish-r11-20261022-windows-x64.zip`，未公开） | tar.gz（历史本地证据：`../artifacts/ui-polish-2026-10-22/packages/ui-polish-r11-20261022/MDcard-0.1.1-ui-polish-r11-20261022-linux-x64.tar.gz`，未公开） |
| 清单文件数 | 22,045 | 21,989 |
| 程序 SHA256 | `c010cb7c1c284e588815e8bb3c8cc04b37521490f7d0642c9479815ee51dc66c` | `a9657f477a5520b609b32c5b349e1235a77c885bd870d59177c1abd59b65d846` |
| 归档 SHA256 | `9e8e20513a2b12ef61d6ecddd8d39e26605d109e45986f78f88f595160bd46fa` | `f20ffc1d87052711dd6270291bcc4bbf1270488ca621a6212bfd8c8e0ad278fc` |

完整包包含离线运行时。运行根目录程序时保留同级 renderer、runtime、assets 等目录。两平台逐文件 manifest、归档成员、哈希及 Linux 执行权限经主代理独立核对，见 包身份汇总（历史本地证据：`../artifacts/ui-polish-2026-10-22/packages/ui-polish-r11-20261022/packages.json`，未公开） 与 独立包审查（历史本地证据：`../evidence/ui-polish-2026-10-22/master/packages-r11.json`，未公开）。原发行包与中间候选保留。

## 九项验收矩阵

“通过”只覆盖证据列明的环境与路径；子项中明确列出的失败与未验证状态保留，不扩展为跨平台或所有数值门槛通过。

| ID | 标准 | 当前状态 | 证据与边界 |
| --- | --- | --- | --- |
| UI-01 | 设计规范统一 | 通过 | 共享 `src/theme.rs` tokens/控件、设计规范、静态400字体与OFL来源；对比度回归与六语言304条字体覆盖通过。 |
| UI-02 | 主界面层级清楚 | 通过 | R11六语言×两主题×18实图共216张，原程序同夹具18张；主代理身份/尺寸核对及中文Split、俄语窄窗导出视觉抽查通过。 |
| UI-03 | 编辑阅读舒适 | 通过 | 排版、三模式、宽表格回流与800窄窗退化通过；最终同包真实公式/Mermaid编辑、源码同步、undo/redo、缩放平移与SVG/PNG通过。 |
| UI-04 | 主题与状态完整 | 通过 | Light/Dark/System×6强调色18组原生点击及持久化、ReduceMotion往返、窄窗Split真实禁用、焦点/dirty/Ready状态；System仅验证当前OS外观。 |
| UI-05 | 窗口与缩放适配 | 通过 | 设备能力组合：Windows真实125%下800×600/1280×800/1920×1080；补充432项egui布局矩阵覆盖1/1.25/1.5/2与六语言/三模式/两主题。物理100/150/200%未验证。 |
| UI-06 | 语言与键盘交互 | 通过 | R11主文档10项、副文档5项真实选区复制、History两层Esc、dirty Quick取消及成功导航；真实微软拼音组合/提交/取消/跨Preferences与模式切换、精确保存通过。 |
| UI-07 | 文档安全与功能 | 通过 | R11同包Linux New未保存保护、副文档replace/undo/redo/主文档保护；Windows开编存/另存/dirty取消放弃/undo精确SHA、BOM/CRLF、编码/结构键盘路径、富节点与双平台九格式导出全部通过。 |
| UI-08 | 响应与实现质量 | 通过 | fmt/严格Clippy、Linux115通过与3专项ignored、Windows原生110/110、独立专项50/52/24通过；全部保留ABBA分布、R11、transition和短程内存综合未见新增持续退化。输入首次基线及部分严格max门槛失败保持原状态，见下文。 |
| UI-09 | 交付证据完整 | 通过 | 最终双平台完整包、来源提交、manifest/归档SHA、216张同条件实图与逐项记录完整；独立全量包审查、Windows/Linux同包实际运行及关键功能验证通过。 |

## 证据索引

证据根目录：evidence/ui-polish-2026-10-22（历史本地证据：`../evidence/ui-polish-2026-10-22/`，未公开）。

| 内容 | 记录 |
| --- | --- |
| 最终独立汇总 | `master/acceptance-r11.json`，15项独立检查全部通过，九项标准按证据环境内通过，失败子项/未验证字段单独保留 |
| 前后对照 | comparison（历史本地证据：`../evidence/ui-polish-2026-10-22/comparison/`，未公开）、before（历史本地证据：`../evidence/ui-polish-2026-10-22/before/`，未公开）、after（历史本地证据：`../evidence/ui-polish-2026-10-22/after/`，未公开），同文档SHA `bb6816d5a02de32e477af77da7ae847ebea41e2a51ded94304c2dd541da5d02c` |
| 最终实图身份/尺寸 | `reports/visual-matrix-r11.json`、`master/visual-review-r11.json` |
| 外观状态 | `reports/states-r11-stable-input/result.json` |
| Windows真实焦点与IME | `master/focus-r11-master/`、`secondary-focus-r11-master/`、`ime-r11-master-transitions/`、`quick-cancel-r11-master/` |
| 原生窗口控制 | `master/window-controls-r11-package/result.json`，最大化/还原、最小化/恢复、标题拖动、红关闭Cancel/Discard与磁盘保护通过 |
| Windows原生测试 | `master/core-windows-final-normalized/`，110/110，测试EXE SHA `fd821c7d2aec7b62e433a888be82ccd03e76f7284018b992b8ade82ba4d21538` |
| 编码/结构 | `master/encoding-structure-r11-package-keyboard/result.json` |
| Linux同包X11 GUI | `master/linux-package/result.json`，真实New/保存保护与双文档操作，不替代Wayland/真实Linux中文IME |
| Windows同包富节点 | `master/rich-package-r11/result.json`，公式/Mermaid真实编辑、源码同步、undo/redo、缩放平移、SVG/PNG、BOM/CRLF和主题通过 |
| Windows同包文档闭环 | `master/documents-r11-package/result.json`，原生Open/Save As、dirty New取消/放弃、undo+save恢复原SHA、编码字节与clean退出通过 |
| 双平台九格式与超时保护 | `master/export-artifacts-r11.json`，HTML/PDF/PNG/JPEG/DOCX/EPUB/Marp HTML/PDF/PPTX全部通过；独立解析PDF2页/幻灯3页/PPTX3页、DOCX12媒体、EPUB6媒体，图片严格解码、HTML资源嵌入、超时保留原目标 |
| 无头缩放补充 | `reports/headless-layout-matrix.json`、`master/headless-r6.json`，432/432；不计为物理桌面DPI |

编码/结构首次脚本依赖Add row的UI_REPORT名称，但原生small_button未记录该名称，探针查找失败保留在 `master/encoding-structure-r11-package/`。调整验证脚本使用既有用户键盘入口CtrlAltEnter，真实增加一行、CtrlZ原子还原原SHA；未改变产品或移除字节/撤销断言。UTF-8→BOM保持pending dirty，磁盘不变，取消New后状态保留，保存精确含BOM/CRLF。

## 性能结果与限制

同机同文档、禁止并行构建测量；原始、ABBA和最终R11全部样本保留。注入到GPU完成不含物理显示扫描；transition另含脚本/OS/JSON观测成本。

| 测量 | 结果 | 状态 |
| --- | --- | --- |
| R11输入55键 | P95 14.457ms、max16.382ms；精确保存通过 | 相对最初P95 9.379的单轮limit11.379ms失败；相对保留ABBA全部baseline165键P95 13.823的limit16.588ms通过 |
| R11滚动1138帧 | P95 8.359ms、max20.221ms，1帧超过16.667ms | P95相对门槛通过；严格全部帧门槛失败 |
| 首GPU完成 | 原15.806ms→R11 13.685ms | 通过（允许增加20ms） |
| 模式/窗口缩放/主题 | P95 114.448→118.308 / 20.241→22.918 / 109.447→109.741ms，各12次 | 相对`max(10ms,20%)`及200ms观测目标通过；主题包含60ms真实按住，鼠标预停在计时外 |
| 短程内存 | 36操作WorkingSet首/尾：原155.27/156.61MB、R11 154.96/157.30MB | 本次未见持续大幅增长；长期泄漏未验证 |

原始报告：`reports/performance-{before,after-final}/`、`paired-{before,after}-{1,2}/`、`performance-r11-final/`、`transitions-{before,after}/`；主代理独立汇总：`master/performance-review.json`、`master/transitions-review-r11.json`。不得表述为所有严格门槛通过或全平台60fps。

## 保留问题与未验证项

- Windows设备物理100/150/200%缩放、OS外观即时变更、Wayland与Linux真实中文输入法未验证；当前125%实测和无头布局分别记录。
- 输入初始单轮数值门槛、严格max16/16.667ms并未全部通过；全部失败样本保留，未以较快轮次替换。
- 短程WorkingSet观测不构成长期内存保证。
- 当前实测范围内没有遗留的编辑、保存或既有关键操作阻断。未验证环境和性能失败子项继续保留。

## 复现与清理

可复现脚本位于 `scripts/verify-ui-polish-{documents,focus-complete,secondary-focus,quick-cancel,encoding-structure,window-controls,ime,ime-transitions,modes}-windows.ps1`，与实际坐标报告配合验证；隔离PID、UUID、二进制哈希和剪贴板恢复保留。Linux同包X11探针为 `scripts/verify-ui-polish-linux-package.py`，在项目Linux容器中通过 `MDCARD_AUDIT_EXE` 指定上述包内程序；其输出固定为本轮Linux验收目录，复现前保留原证据副本。截图与状态脚本为 `capture-ui-polish-windows.ps1`、`verify-ui-polish-states-windows.ps1`；性能脚本及契约见实现说明。

探针只操作隔离的测试PID并恢复剪贴板，测试程序按报告clean退出。验收时保留了原始失败、R4中间截图、R8旧包与原dist；随后按用户“清理旧资源、保留最新”的要求，移除被替代的程序、旧包和临时探针副本，保留本轮截图、失败记录、最终测试程序与验收证据，详见[资源清理记录](RESOURCE_CLEANUP_2026-10-22.md)。`dist` 已同步最新候选，原有本轮完整包入口保持可用。产品包固定在源码提交`c430a7d`，清理与文档提交不修改已经验收的程序或归档。
