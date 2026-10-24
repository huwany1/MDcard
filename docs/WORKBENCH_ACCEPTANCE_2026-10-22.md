# MDcard 工作台重设计：当前候选验收

本轮视觉审查通过，原生编辑、保存、导航及关键操作回归通过。输入、滚动、搜索和布局切换满足实现前冻结的相对预算。严格的“所有滚动帧小于16.667ms”门槛失败：1099帧中有1帧20.1382ms；物理100%、150%、200% DPI、Wayland及Linux真实输入法未验证。这些结果分别保留，不将相对预算通过写成所有性能/平台项通过。

本记录只适用于下列R9程序及使用相同二进制的 `workbench-final-20261022` 文档发行快照。早期 UI_POLISH、R1–R8及旧dist记录均为历史，不作为本轮最终证据。设计职责图、组件规范、入口清单、操作步骤前后比较和修订理由见 [工作台设计](WORKBENCH_REDESIGN_2026-10-22.md)。

## 程序及完整包身份

| 项目 | 当前身份 |
|---|---|
| 产品源码提交 | `78c657d4dd8c7fca47dce68a54218b78a65f21cd`，分支 `codex/workbench-redesign-2026-10-22` |
| Windows程序SHA256 | `4e54b4eab36e42be653e040e9a00ef1c5691303b1c61d2cc12ac503715f18e3b` |
| Linux程序SHA256 | `5ab14b93b4b64264851a332e6d4d81d9ba2c5f2cc1ae4a4cacdb87c45cce182a` |
| 最终buildId | `workbench-final-20261022`；文档/验证脚本快照提交见manifest的`sourceCommit`，原程序提交另存`binarySourceCommit` |
| Windows完整包 | `dist/MDcard-0.1.1-workbench-final-20261022-windows-x64.zip`，直接运行入口 `dist/windows/MDcard.exe` |
| Linux完整包 | `dist/MDcard-0.1.1-workbench-final-20261022-linux-x64.tar.gz`，直接运行入口 `dist/linux/mdcard` |
| 索引与校验 | `dist/latest.json`、各包`manifest.json`；包含归档与逐文件SHA256。原始候选目录中的重复包及`packages.json`已清理，当前入口统一使用`dist`，详见[清理记录](RESOURCE_CLEANUP_2026-10-22.md) |

最终快照只更新随包说明，沿用已经原生验收和性能测量的R9程序，不重新编译功能。`scripts/sync-workbench-dist.py`校验后复制相同stage与归档，不重新压包。独立归档/分发核验结果由 `evidence/workbench-redesign-2026-10-22/master/acceptance-final.json` 汇总；实际身份以manifest及该独立记录为准。

## 验收矩阵

下表中证据路径均相对于 `evidence/workbench-redesign-2026-10-22/`，属于当前本地仓库证据。完整运行时包带设计及验收说明；大量实图、原始采样和历史失败保留在仓库证据目录。状态仅使用“通过 / 失败 / 未验证”，括号解释边界。

| ID | 验收项 | 状态 | 当前程序证据 |
|---|---|---|---|
| UX-01.1 | 区域职责图、入口清单、调整理由 | 通过 | 工作台设计中的布局、入口与步骤比较；`master/visual-review-r9.json` |
| UX-01.2 | 工作区、文档、活动pane、模式、dirty可识别 | 通过 | `master/navigation-master-r9/result.json`4/4；`dual-final-r9-ru/result.json`及实图 |
| UX-01.3 | 标题/工具/侧栏/状态分工与重复入口用途 | 通过 | Files/Outline/Recent互斥导航；File/Search/更多分组；`after-final/zh-CN-Light/` |
| UX-01.4 | 稳定正文空间、辅助操作任务分组 | 通过 | 三尺寸实图；`master/static-78c657d/layout-matrix.log`；六语言各100帧副窗稳定性回归 |
| UX-02.1 | 复用颜色、字阶、间距、控件/图标/边框/圆角规范 | 通过 | `src/theme.rs`的button/document_header/icon_button/modal及共享原生widgets；设计组件规范 |
| UX-02.2 | 基线、命中区域、对齐和状态一致 | 通过 | `master/visual-review-r9.json`；`component-states-final-r9-menu/result.json`；source/preview36px文档头与局部行 |
| UX-02.3 | 强调色、文字层级、重要状态兼有文字/形状 | 通过 | `reports/states-final-r9/result.json`18状态；active与dirty同时显示；双窗统计标明主文档 |
| UX-02.4 | 审查秩序、重心、密度、排版、一致性并保留批注 | 通过 | 下文截图批注及设计R1–R9修订；主代理独立8张关键图审查 |
| UX-03.1 | 标题/工具/树/大纲/源码/预览/副稿/状态覆盖 | 通过 | 252张六语/两主题实图、3000大纲、空工作区与窄dirty副稿专项 |
| UX-03.2 | Quick/查找/全局搜索/替换预览/结构/编码/导出/设置统一 | 通过 | `visual-final/result.json`；`master/focus-master-r9/result.json`；原生搜索/结构/编码与导出生命周期 |
| UX-03.3 | 弹窗标题分组、主次动作、退出与窄窗动作可达 | 通过 | `after-final/*/compact-*.png`；主代理俄/日/韩抽查；Esc/Cancel/Done焦点回归 |
| UX-03.4 | 路径、长名截断与完整内容可查看 | 通过 | `navigation-long-path.png`、`master/navigation-master-r9/05-expanded-long-path.png`；路径tooltip；俄语最窄dirty双窗 |
| UX-04.1 | 打开文件夹、编辑保存、三模式、副文档、搜索、预览替换、设置、导出实际完成 | 通过 | 导航4/4、文档10项、焦点10/10+5/5、原生结构/编码与富节点、两平台9格式实际产物 |
| UX-04.2 | 入口、当前状态、完成反馈、返回位置明确 | 通过 | 下文原生逐帧流程；搜索/导出进度与结果、dirty保护；主代理视觉/功能复核 |
| UX-04.3 | 关闭/取消恢复文档、焦点、精确选区及合理滚动位置 | 通过 | `master/focus-master-r9`、`master/secondary-focus-master-r9`、Quick取消与原生folder取消的Ctrl+C哨兵 |
| UX-04.4 | 常用流程前后步骤、入口发现变化有证据 | 通过 | 设计文档“最终入口清单与操作步骤比较”；同条件前后图及逐帧原生流程 |
| UX-05.1 | 默认/悬停/按下/选中/焦点/禁用/加载/空/错误按用途覆盖 | 通过 | `component-states-final-r9-menu`悬停/按下/regex错误恢复/Quick空；18状态中的窄窗Split禁用；导航/空状态；实际导出进度/取消及后台查询 |
| UX-05.2 | 图标名称/快捷键与禁用原因 | 通过 | Save等tooltip实图；共享icon_button命名；OpenFolder忙态原因与窄窗Split说明；布局矩阵 |
| UX-05.3 | Esc/Enter/Tab/既有快捷键与模态层级 | 通过 | 主/副焦点15项；原生Quick的Enter/Cancel/Esc；`master/encoding-structure-final-r9`实际`Tab Tab Space`选择BOM并保存精确字节；结构快捷键及单步撤销 |
| UX-05.4 | 真实中文IME组合/提交/取消不重复或误改稿 | 通过 | `master/ime-final-r9-verified/result.json`，Microsoft Pinyin TSF预编辑、提交一次、Esc取消、模态/模式转换及精确保存 |
| UX-06.1 | 800×600、1280×800、1920×1080与侧栏/副稿/连续resize | 通过 | 12组原生窗口；双稿专项与原生resize焦点；432逻辑布局组合 |
| UX-06.2 | 窄窗按可用空间收纳、核心功能仍可达 | 通过 | 窄窗File/Search/更多菜单；编辑/阅读/保存；副窗宽度稳定；compact各panel实图 |
| UX-06.3 | 长表格/代码/路径局部溢出与正文自适应 | 通过 | `rich-final-r9`源码/节点；3000标题长文；长路径实图；共享scroll区域与432布局 |
| UX-06.4 | Light/Dark/System、既有强调色、六语主界面及主要面板 | 通过 | 252PNG；`reports/states-final-r9/result.json`18状态；317语言key集合与占位符一致性；独立实图审查 |
| UX-06.5 | 100/125/150/200%应用模拟与真实DPI分别报告 | 通过 | `master/static-78c657d/headless-layout-matrix.json`432逻辑组合；原生报告实际125%，其余真实DPI另列未验证 |
| UX-06.6 | 100%、150%、200%真实系统DPI | 未验证 | 未改变宿主桌面物理DPI；无头比例模拟不代替此项 |
| UX-07.1 | 纯视图操作保持源码字节及dirty | 通过 | 12组visual源SHA不变、18状态、富节点三模式/缩放/平移；原生字节闭环 |
| UX-07.2 | 新建/打开/保存/另存为/关闭保护/外部修改及失败保护 | 通过 | `master/documents-master-r9-native/result.json`10必需bool全true；当前120核心测试涵盖外部冲突/原子写入失败 |
| UX-07.3 | 主副焦点/撤销隔离、编码/结构/检索/富节点/导出 | 通过 | 主10/副5焦点；编码结构原生；Linux副稿替换/撤销；公式图表与两平台9格式；当前核心模块回归 |
| UX-07.4 | 不承诺未实现自动保存/备份/崩溃恢复 | 通过 | 界面与README描述实际显式保存保护；本轮未增加恢复承诺 |
| UX-08.1 | 冻结同条件性能基线/容差并验证输入/滚动/搜索/切换 | 通过 | `reports/performance-comparison.json`7个相对门槛全通过；详细表见下 |
| UX-08.2 | 失败样本、计时边界与超标披露 | 通过 | 原始result/100轮records全部保留；严格滚动子项失败单列；无物理显示延迟声称 |
| UX-08.3 | 严格全部滚动帧<16.667ms | 失败 | 最终1/1099帧20.1382ms；基线3/1094帧，max32.3192ms；未按P95抹除失败 |
| UX-08.4 | 构建、格式、静态、功能验证 | 通过 | `master/static-78c657d`：120通过/0失败/3ignored，fmt/clippy exit0；显式432 ignored矩阵另跑通过 |
| UX-08.5 | 同文档/尺寸/主题/比例前后图及关键流程演示 | 通过 | `before-workspace/zh-CN-Light`与`after-final/zh-CN-Light`；下文批注、逐帧流程及证据浏览页 |
| UX-08.6 | 程序/完整包/源码/buildId/manifest/SHA对应 | 通过 | 最终包的`sourceCommit`/`binarySourceCommit`，主代理归档与dist逐文件独立核对，见`master/acceptance-final.json` |

## 同条件截图批注与逐帧流程

主展示比较使用同一多层工作区夹具、同一示例文件、Light/zh-CN/1280×800逻辑尺寸、实际125%系统DPI和OpenGL后端。指针移至中立处，展示图没有搜索tooltip遮挡文档名。`before/`的一文件初查仅辅助说明，导航比较以三文件 `before-workspace/` 为准。

| 条件与实图 | 批注及修订效果 |
|---|---|
| 改造前分屏（历史本地证据：`../evidence/workbench-redesign-2026-10-22/before-workspace/zh-CN-Light/split-1280x800.png`，未公开） / 改造后分屏（历史本地证据：`../evidence/workbench-redesign-2026-10-22/after-final/zh-CN-Light/split-1280x800.png`，未公开） | 两个大检索块移走、导航三视图分开；工具层压缩，正文增加共享身份行；源码/预览操作按同一行高对齐。名称左对齐、H1层级恢复，渲染器诊断离开状态栏主体。 |
| 改造前设置（历史本地证据：`../evidence/workbench-redesign-2026-10-22/before-workspace/zh-CN-Light/preferences.png`，未公开） / 改造后设置（历史本地证据：`../evidence/workbench-redesign-2026-10-22/after-final/zh-CN-Light/preferences.png`，未公开） | UI标题由局部20px控制，不影响文稿标题；设置按主题/编辑/运行配置分组；长内容滚动与固定退出动作适配窄窗。 |
| 改造前窄查找（历史本地证据：`../evidence/workbench-redesign-2026-10-22/before-workspace/zh-CN-Light/compact-find-replace.png`，未公开） / 改造后窄查找（历史本地证据：`../evidence/workbench-redesign-2026-10-22/after-final/zh-CN-Light/compact-find-replace.png`，未公开） | 根目录改basename及完整tooltip，作用域/筛选/替换/结果分组，长根路径不占整条输入行。 |
| 大纲（历史本地证据：`../evidence/workbench-redesign-2026-10-22/after-final/zh-CN-Light/navigation-outline.png`，未公开） / 最近（历史本地证据：`../evidence/workbench-redesign-2026-10-22/after-final/zh-CN-Light/navigation-recent.png`，未公开） / 长路径（历史本地证据：`../evidence/workbench-redesign-2026-10-22/after-final/zh-CN-Light/navigation-long-path.png`，未公开） | 导航不再三个独立小高度堆叠。大纲独占侧栏并保持全局源码定位；会话最近历史有独立入口；多层名称截断及完整路径提示。 |

可打开 `evidence/workbench-redesign-2026-10-22/review.html` 查看原始PNG的前后切换和关键流程逐帧演示；它是截图序列，不宣称连续屏幕录像。原始JSON记录精确文本、字节、焦点、选区、进程和程序hash，图像用于解释状态与布局。

- 文件夹与副稿：`master/navigation-master-r9/01`原生取消保留draft → `02`新workspace同时保留dirty原稿 → `03/04/05`大纲/最近/深层路径 → `06`原稿撤销保存仍原SHA → `07`主阅读点击从副稿夺回活动目标。
- 文档保护与保存：`master/documents-master-r9-native/01`撤销回clean → `02`新建未保存保护 → `03`先确认真实草稿精确内容再原生SaveAs → `04`原生打开BOM/CRLF原稿；10项断言及正常退出全部通过。
- 编辑焦点返回：主稿10/10、副稿5/5，覆盖Preferences、编码、Quick、查找、结构、导出、历史、视图、侧栏、resize；以真实Ctrl+C哨兵验证精确选区，不以“focused非空”代替。
- 编码与结构：当前包先设置UTF-8 BOM但不写盘，取消保留dirty，再保存BOM/CRLF；结构添加表格行后一次Undo回原字节。
- 公式图表：`rich-final-r9`真实本地引擎渲染、修改/撤销/重做、预览/源码/分屏、缩放平移、SVG/PNG导出。两平台完整包9格式另有文件解析验证。
- 导出生命周期：实际UI导出至原生SaveAs后取消、关闭、重试及原生关窗；当前进程树回收、旧目标/稿件/无关进程保持，重试完成30083bytes HTML。取消回收实测约2.60–2.77秒，不声称瞬时终止。

## 实测性能与边界

Windows同机 NVIDIA RTX4060 Laptop / OpenGL，reduce motion，15px。55次真实SendInput以跨进程QPC从注入计至GPU完成；滚动使用原生滚动条并保留全部帧。该边界不包含物理扫描输出，不等于输入到屏幕光子的延迟。

| 指标 | 基线(ms) | 最终(ms) | 冻结上限(ms) | 状态 |
|---|---:|---:|---:|---|
| 输入→GPU P95 | 8.8643 | 9.3257 | 10.8643 | 通过 |
| 滚动GPU帧P95 | 8.2525 | 8.3714 | 10.2525 | 通过 |
| 文件打开→首GPU完成 | 15.4352 | 12.8534 | 35.4352 | 通过 |
| 模式切换观测P95 | 139.4235 | 131.1252 | 167.3082 | 通过 |
| resize观测P95 | 26.3577 | 25.5532 | 36.3577 | 通过 |
| 主题切换观测P95 | 110.2671 | 100.0451 | 132.3205 | 通过 |
| 搜索可见P95 | 12.0080 | 10.7230 | 17.0080 | 通过 |
| 严格滚动全部帧 | max32.3192，3/1094超标 | max20.1382，1/1099超标 | 每帧<16.667 | 失败 |

模式/resize/主题各12次，计时包含系统事件、report提交与轮询，主题脚本含60ms按下；报告另存update CPU样本，不将该观测值写成纯函数耗时。搜索同1600文档13.67MB、100轮、约148.70s，visible max11.6345ms、worker max6.0802ms，增量增删与周期全字节复检均发生，无并行CPU负载进程。短程内存取样不能证明长期无泄漏。最终性能测试期间未并行编译、打包或其他GUI测试。

原始证据为 `reports/performance-before`、`performance-final-r9`、`transitions-before/after`、`search-before/after`；汇总脚本只读这些原始样本，不排除慢帧。最终性能脚本因严格滚动子项返回exit1，属于明确保留的失败；功能及相对预算仍分别通过。

## 平台、输入法和已知限制

| 范围 | 状态 | 证据与边界 |
|---|---|---|
| Windows原生OpenGL与125%物理DPI | 通过 | 当前包252图、15项焦点、保存/导航、真实Microsoft Pinyin TSF、原生窗口控制和性能 |
| 100/125/150/200%应用逻辑布局 | 通过 | 显式432组合测试，三尺寸/六语言/两主题/三模式；不能代替物理DPI |
| 物理100/150/200%及跨显示器DPI迁移 | 未验证 | 未切换宿主DPI或不同显示器 |
| Linux Debian12 X11/Xvfb/Openbox/Mesa软件渲染 | 通过 | `master/linux-r9`真实原生pending-edit保护、副稿替换与undo/redo；完整包本地渲染与9格式产物 |
| Linux真实桌面硬件GPU、Wayland、真实IME | 未验证 | Xvfb与无头运行不推定真实桌面/输入法行为 |
| macOS程序或Apple平台认证 | 未验证 | 本轮借鉴视觉语言，交付目标为Windows和Linux |
| 严格所有滚动帧<16.667ms | 失败 | 1099帧中的1个慢帧20.1382ms |
| 物理显示时延 | 未验证 | 未使用光电/ETW显示呈现测量 |

真实Pinyin验证通过“预编辑存在、提交恰好一次、取消不改已提交正文、转换后无孤儿预编辑、精确保存”。egui/TSF组合期间可暂将预编辑反映在live buffer；不承诺预编辑永不进入缓冲。最初额外的 `preedit_not_in_source=false` 报告保留在 `master/ime-final-r9`，它不属于冻结成功标准；修正断言后的最终报告由主代理逐字段和截图复核。其他输入法或OS组合未由此推定。

本轮保留既有行为：不新增多标签、任意停靠、自动保存或崩溃恢复；副文档编码功能不扩展，状态栏显式归属于主文档；空目录和不支持文件类型不列入树。打开工作区为安全导航操作，替换事务忙时拒绝；原生busy期间picker行为由核心守卫及禁用文案保证，未以并发系统选择器测试声称通过。无用户研究样本，入口发现性的结论来自可见入口/步骤比较及审查，不声称量化用户效率提升。

## 历史失败与最终独立复核

R1原生点击死锁、R7副稿宽度漂移均为真实产品阻断，已经修复并加入定向绘制回归；原稿字节保持。R9第一次SaveAs测试未成功向新稿粘贴，产生0字节草稿，保留原报告；驱动加入真实点击停留、前台所有权与粘贴后精确文本断言，任何必需bool为false即失败。最终 `documents-master-r9-native`10项全true，并非仅按exit0认可。原生picker旧Alt+S输入路由、剪贴板争用等失败同样保留，不伪装为业务验证。

补充组件态测试的前两轮在手工按下截图后未成功打开全局检索，`component-states-final-r9`及`component-states-final-r9-verified`失败保留。成功证据以经过可见Search菜单并先断言窗口open的`component-states-final-r9-menu`为准；既有原生搜索性能/焦点套件已独立覆盖Ctrl+Shift+F，不由这些驱动失败推定领域搜索错误。

主代理独立验证当前120核心测试、fmt/clippy、432矩阵，逐字段核验原生保存/导航/15焦点、真实IME、编码结构、Quick取消、窗口控制、空/双稿、18状态、富节点与两平台9格式。它另逐文件检查完整包manifest/归档、二进制身份及最终dist同步，视觉与功能分别给出结论，索引见 `master/acceptance-final.json`。没有尚未修复的阻断编辑、保存、导航或关键操作问题；上表明确保留的性能失败与平台未验证项不转写为通过。
