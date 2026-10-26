# 稿件崩溃恢复本次验收 · 2026-10-26

验收范围为当前未提交工作区的本地实现和独立候选，不发布、不替换正式包。最初已有自动保存、文档模型、结构编辑等未提交改动均延续保留。本页以本次实际运行的证据判断完成，不用历史发行验收代替。

C01–C12 已在下述本地验证范围内完成。最终交付为 **final-r4**，四套原生脚本均使用同一普通候选。先前候选和失败运行仅作为排查证据，不代替本轮结果。

本地总索引：[final-summary.json](../evidence/recovery/2026-10-26/final-summary.json)。独立主代理逐项核对了 116 个源码输入和两份可执行文件：[最终身份审计](../evidence/recovery/2026-10-26/master-final-identity-audit.json)。

## 环境与候选

- Windows 11 家庭版中文版 x64（10.0.26100）原生窗口、已解锁桌面、125% DPI；操作仅针对脚本启动的 PID，使用真实键鼠与剪贴板，结束时恢复剪贴板和进程环境。
- Linux 构建/单元与独立回归：`mdcard-dev` 容器，Debian GNU/Linux 12 x86_64，Rust 1.92.0、Cargo 1.92.0。
- Windows 交叉构建目标：`x86_64-pc-windows-gnu`。普通候选关闭故障特性；另有 `acceptance-faults` 候选，仅用于受控表格 patch 失败。
- 原生渲染记录：OpenGL 3.3.0 NVIDIA 596.08，NVIDIA GeForce RTX 4060 Laptop GPU；详见 Core 的 `01-main.json`。
- HEAD：`dd168ff02e0791e8e9260c121c004cde3fac4800`。工作区含本次及先前未提交改动，不能仅以 HEAD 标识候选。完整 runtime 源文件指纹及逐文件 SHA256 随每份 `candidate-manifest.json` 保存。

最终候选目录：`artifacts/recovery-2026-10-26/final-r4/`。

| 身份 | 实际值 |
| --- | --- |
| 当前 runtime 源码指纹 | `53358bb135ea5bd9653885486456e1fe5ea8cf1fe7405b16d1b522fcd989b433` |
| 普通候选 SHA256 | `c84c8ce1aeca3f3d80e3676acbf0d10097cad6ec113c6384326eac6e883a5d64` |
| 受控故障候选 SHA256 | `9f94c4fefb6c024a9eacc9d77ad497f5b9f0cf1d0dfcc1a3727549bd4e49d599` |
| 清单 | `artifacts/recovery-2026-10-26/final-r4/candidate-manifest.json` |

## 本次自动化

| 检查 | 本次已执行结果 | 证据 |
| --- | --- | --- |
| r4 根测试 | 275 passed、0 failed、6 ignored | `evidence/recovery-root-r4.log` |
| 恢复生产 API 定向测试 | 29 passed、0 failed、3 ignored；随后 r4 根测试再次覆盖 | `evidence/recovery-tests-r3.log` |
| 正常 Cell 同步后草稿清理及保存 | 1 passed、0 failed | `evidence/recovery-table-regression-after.log` |
| Linux 全目标、全部特性 Clippy | 退出 0 | `evidence/recovery-clippy-linux-r4.log` |
| Windows 全目标、全部特性 Clippy | 退出 0 | `evidence/recovery-clippy-windows-r4.log` |
| Linux release 构建 | 退出 0 | `evidence/recovery-build-linux-r4.log` |
| 七个独立 Rust 回归工程 | 本次整链退出 0，包括搜索替换/回滚、性能、rich 节点、阅读状态与精度；结构编辑由当前根测试和原生用例覆盖；详细逐工程计数保存在日志 | `evidence/recovery-final-linux.log` |
| 真实 OS 磁盘满 | 在有界 `/dev/shm` 的隔离临时目录触发 OS28；上一有效版本保留 | `evidence/recovery/2026-10-26/storage/enospc.log` |
| 真实 OS 权限不足 | uid65534 触发 OS13；上一有效版本保留 | `evidence/recovery/2026-10-26/storage/permission-denied.log` |

默认 ignored 包含需要显式启动的子进程 helper、OS 失败专用入口和既有专用矩阵，不能计作通过；实际父用例已启动真实子进程，并验证活跃实例、强制结束、竞争领取和写入中断。OS 两个专用用例也另行明确执行，记录其真实错误码。

| 补充检查 | 本次已执行结果 | 证据 |
| --- | --- | --- |
| Windows 原生恢复专项 | 29 passed、0 failed、2 ignored，含真实进程与三个强杀中断点 | `evidence/recovery-tests-windows-r4.log` |
| Windows 原生跨块源码选区后 Cell 编辑/保存 | 1 passed、0 failed | `evidence/recovery-table-windows-r4.log` |
| 格式检查 | 退出 0 | `evidence/recovery-master-fmt-r4-result.json` |
| Windows 普通/故障候选、测试程序构建 | 退出 0 | `evidence/recovery-final-candidates-r4.log`、`evidence/recovery-test-build-windows-r4.log` |
| Git 差异格式检查 | `git diff --check` 退出 0 | `final-summary.json` |

上述补充行与前表共同构成本次检查清单。格式检查时曾有同名工具日志占用导致一次重定向未启动，已单独标记为 `recovery-fmt-tool-log-collision.json`；它不作为 fmt 运行失败或通过的证据。

## 原生程序验证

| 套件 | 验证 ID | 实际目录与覆盖 |
| --- | --- | --- |
| Core | `0a1a1bdc-1213-4012-9c5e-2403260943af` | `evidence/recovery/2026-10-26/windows-core-024228/`；主副、未命名、空正文、恢复后二次强杀、外改/删除、取消关闭、稍后、删除确认、开关持久化、失败重试与最终无空提示 |
| Drafts（已提交输入） | `a2fb9182-fefd-44c9-8ac5-4b60efca36b8` | `evidence/recovery/2026-10-26/windows-drafts-030348/`；7 个强杀恢复 case，16 步骤，含正常表格手动/自动保存及无误报重启 |
| Performance | `ccdaea1c-8e12-47c8-8570-8807f07c249e` | `evidence/recovery/2026-10-26/windows-performance-025636/`；1/8/16 MiB，连续提交字符、滚动、切视图与手动保存 |
| 原文件自动保存回归 | `cd56f8bf-a3bc-4ddb-bbec-e588400f4bb2` | `evidence/recovery/2026-10-26/autosave-025915/`；默认关闭、明确开启、旧 false 恢复、主副独立、只读重试、退出保护、外改冲突、另存为 |

Drafts 的普通候选完成有效/无效公式、有效/无效 Mermaid、源码实际变化引起的 rich 冲突、正常表格；表格未同步负路径明确使用故障候选及 `MDCARD_ACCEPTANCE_TABLE_PATCH_FAILURE=1`，只让现有 patch 返回错误，其余真实输入、独立草稿、存储、强杀、读取、界面恢复和复制均走同一生产路径。正常表格 case 已明确断言成功同步后 `drafts=0`，保留复制正文后的跨块选区，随后手动保存和自动保存都清理恢复需求，重启无误报。

连续输入采用反复单字符剪贴板粘贴，代表已提交给应用的文字，不切换、不强制结束用户输入法。较早用 SendKeys 字母的性能运行在实际 IME 组合状态下被手动保存保护正确阻止；恢复仍提交了当前检查点，失败目录保留。该轮没有记录原始 IME 事件，不能据此声称末尾是空 Preedit，也不将该失败冒充性能通过。

## C01–C12 证据表

| 标准 | 生产路径与本次证据 | 最终 r4 状态 |
| --- | --- | --- |
| C01 默认与设置 | `preferences` 默认/缺字段/显式 true、false 往返；设置失败保留会话选择；Core 开关持久化、关闭后保留旧副本并停止新检查点 | 通过 |
| C02 正文及格式 | 未命名、同路径主副、Unicode、混合换行、全部删除、编码/BOM的真实快照；Core 从界面恢复并核对 UTF-8 长度/指纹及可见源码复制 | 通过 |
| C03 独立草稿与输入法边界 | Drafts 的有效/无效公式、有效/无效图表、自然源码变化引起的 rich 冲突；普通表格与独立故障候选的未同步表格分别强杀；独立草稿通过界面复制核对；状态测试确认组合期间不停止已提交内容保护 | 通过 |
| C04 时序及中断 | 可控 2 秒/10 秒与延迟 completion 周期；写临时文件、文件同步、原子提交三个位置强杀真实子进程；损坏新世代、损坏指针与预约计数回退验证 | 通过 |
| C05 身份与顺序 | 会话/缓冲区的 128 位随机身份、预约令牌、同名/同路径独立缓冲区、主副、Save As、rename、受控迟到结果与重新打开；预约从有效世代重建单调顺序 | 通过 |
| C06 恢复流程 | Core 列表、固定稍后按钮、恢复为无路径 dirty 副本、删除确认与取消、再次崩溃、无记录无空提示 | 通过 |
| C07 原文件保护 | 生产安装处理覆盖未变、外改、删除、移动、只读、不可访问；Core 即使原文件自动保存开启仍不写回旧路径，并核对原字节哈希 | 通过 |
| C08 生命周期 | 保存 A 保留 B、失败保存、关闭取消、明确放弃及打开失败、恢复草稿单独保留；清理失败主界面可见并阻止静默退出；正常 Cell 同步后不再误留草稿 | 通过 |
| C09 多进程 | 两平台真实进程活跃占用、kill 释放、两个竞争领取者只有一个成功；内核租约不依赖 PID/时间年龄 | 通过 |
| C10 容量/失败/隐私 | 16 MiB 正文及草稿文本、128 草稿、128 稿件、128 MiB 序列化、512 MiB 总量；未知/截断/异常大小、OS28/OS13；恢复诊断正文置空，元数据只保留长度/缓存指纹 | 通过 |
| C11 交互与性能 | 各窗格状态、错误原因与重试、有界任务；六语言文案；1/8/16 MiB原生输入、滚动、切视图、手动保存、工作集/帧/队列采样 | 通过，影响与测量盲区见下表 |
| C12 本次交付 | fmt、Clippy、根测试、相关独立回归、Linux/Windows 构建、当前候选原生强杀流程、使用说明及可复跑入口 | 通过 |

## 验证中发现并修正的问题

1. Windows 诊断接口使用截断后写入，脚本曾将空读误当成空列表或控件消失。受控复现中，400 次普通读出现 152 次 null 和 6 次无效 JSON；`ReadJsonStable` 连续 200 次读取完整状态。脚本现在校验必需字段并在同一快照查找控件/身份，恢复数据本身的原子格式未因此更改。证据：`evidence/recovery/2026-10-26/json-reader/result.json`。
2. 高负载下 Store 锁竞争会排队重试，旧测试 helper 只等 worker 结束而没等待 `retry_after`，造成一次根测试失败。新增持锁超过 500ms 的生产路径用例，验证释放后两窗格均成功保护；helper 明确推进逻辑时钟并等待重试。本轮根测试已重新完整通过。
3. 截断/回退的预约计数原先可能让成功提交的 C 排在旧 B 之前。失败测试真实得到 B，r3 改为从有效世代最大序号与预约重建顺序，并通过随机令牌拒绝迟到提交。证据：`evidence/recovery-sequence-regression-before.log` 与 `evidence/recovery-tests-r3.log`，随后 r4 根测试再次覆盖。
4. 正常 Cell 成功同步后原先可能残留同值草稿并阻塞自动保存。失败测试已复现，r3 仅清理与成功 patch 匹配的草稿，保留失败和其他输入；原生正常表格另行核对手动/自动保存后不误报恢复需求。

5. 原生“复制正文后编辑单元格”会留下源码跨块选区，原本统一的结构范围检查错误阻止了有明确坐标的 Cell。失败用例已复现，r4 仅豁免显式 Cell，保留其他命令的原有跨块限制。当前根测试和 Windows 原生用例均通过，Drafts 也在不清掉该源码选区的条件下完成自动保存。

旧失败目录与较早候选保留为本次排查记录，不能作为 r4 完成依据。

## 原生性能与交互

原始入口为 `verify-recovery-windows.ps1 -Suite Performance`；关闭了会逐帧序列化普通正文的 `MDCARD_SEARCH_REPORT`，保留 UI、性能、rich/structure 元数据报告。测量时没有并发构建或 Linux 长回归。原始数组、120 次输入期间的工作集/队列采样、每份报告与截图保存在 `windows-performance-025636/`，统计由 `scripts/summarize-recovery-performance.py` 生成，无慢样本筛除。

| 稿件 | worker 成功回执 n / 中位 / 最大 ms | 输入更新 n / 中位 / p95 / 最大 ms | 全更新窗口最大 ms | 采样峰值 / 原生进程峰值 MiB | 观测 writer / 待处理版本峰值 |
| --- | --- | --- | --- | --- | --- |
| 1 MiB | 4 / 23.96 / 25.45 | 126 / 3.58 / 4.61 / 4.96 | 173.06 | 301.8 / 307.2 | 0 / 0 |
| 8 MiB | 3 / 49.91 / 76.73 | 127 / 13.56 / 21.88 / 81.17 | 81.17 | 921.1 / 961.1 | 1 / 0 |
| 16 MiB | 4 / 128.93 / 273.10 | 126 / 22.97 / 26.93 / 80.64 | 145.16 | 1481.3 / 1561.3 | 0 / 0 |

worker 时间在后台从存储工作开始到文件/元数据提交完成测量；`snapshot_at` 是快照时间，`scheduled_at`、worker `committed_at`、UI `observed_at` 分别记录，不把调度或轮询时间当成实际提交。此表采集到的成功回执为 4/3/4 次，并非声称收集了所有后台任务。

输入帧指标是事件进入应用后至 update 完成的 CPU 时间，不包含 OS 投递、GPU 呈现或显示扫描。全部 update 指标使用现有接口末尾 **2048 条环形保留窗口**，不声称覆盖完整启动历史；输入数组的 126/127/126 条均被纳入。快照复制本身仍有 UI 开销，序列化和磁盘工作在后台。

8 MiB 采样确实观察到 writer 在途时发起手动保存，后续保存、内容及未保存状态断言通过；1/16 MiB 未在采样点撞到 writer，不把观测到的 0 解读为没有备份，也不声称三种尺寸全部测到了同时保存。实现上限仍为每窗格 1 writer、无排队的完整写入快照、每窗格 1 份最新版本信息；主副并发、忙重试与迟到结果另有生产测试。

**16 MiB 输入更新最大 80.64 ms、全部更新窗口最大 145.17 ms，进程峰值约 1561 MiB。** 这不是“无影响”或“低内存”结论。工作集是整个应用进程的测量，未做关闭恢复保护的 A/B 对照，不能拆成恢复功能的内存增量或归因给某一模块。此轮持续输入、滚动、切视图和正常保存均完成且内容校验通过，但不提供稳定 60fps 或更低内存保证。

## 未验证范围

真实执行的故障是进程强制结束，以及可控存储/同步/提交中断和内核返回的 tmpfs 空间不足；没有执行系统崩溃、虚拟机硬复位、物理断电或硬件缓存丢失。Windows 的文件同步/原子替换与 Linux 目录同步证据不等于已验证物理掉电保证。未执行本轮 macOS、其他 Linux 桌面、其他 DPI/驱动和全部实体输入法组合窗口测试。恢复承诺以最后成功提交的检查点为准，持续运行但尚未提交的字符不被夸大为已备份。
