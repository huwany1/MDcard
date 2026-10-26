# 大稿件内存诊断与优化验收（2026-10-26）

本次是稿件恢复交付之后的内存跟进，保留全部既有未提交改动和 `artifacts/recovery-2026-10-26/final-r4`，不替换发布包。原文件自动保存仍为新设置默认关闭、恢复保护默认开启；用户已保存的 `autosave:true` 继续生效。所有测量另建隔离设置，固定 `autosave:false`，不修改真实用户设置。

## 来源与实现

本次独立原生诊断先在原候选做恢复开/关对照，没有把 1561 MiB 直接归因于恢复。进一步对 Tree-sitter 安装其官方 `set_allocator` hook，以 requested bytes 计数 C core 分配；Rust 侧独立统计 requested heap，二者都不等于进程工作集或 PrivateBytes。

16 MiB 连续软换行段落的独立 Parser 试验产生 202140 个树节点、546.18 MiB C core requested live bytes；释放 Tree 后降至约 0.25 MiB，释放 Parser 后归零。原生持续编辑时 C core live 达到 1066681424 B，峰值 1093409576 B，说明旧、新全文树共同驻留是主要来源；不是仅凭源码 clone 次数推测。Rust 当时 live 约 329 MiB、历史峰约 499 MiB；其分组是分配来源，值跨线程释放时保留原组。小于 4096 B 的分配与未命名线程归第 0 组，不能将第 0 组解释为单一 UI 模块。各组历史峰不同时发生，不能相加得到进程峰值。参考：`evidence/memory/tree-parser16-before.json`、`evidence/memory/2026-10-26/profile-before16-r3/16-on/ui.json`。

全篇 Tree-sitter 后端现在限制在 1 MiB；更大的全文通过已有 CommonMark parser 消费事件，仅保留顶层块、引用定义的原文范围。安全局部编辑的旧、新上下文均不超过 8192 B 时继续使用 Tree-sitter 增量语法校验；其余变更在同一后台 worker 扫描边界。平衡 AST 分页借用事件文本，避免整篇 `into_static` 文本副本。1 MiB 是后端选择，仍支持 16 MiB 稿件；不缩减恢复正文或草稿、不调整 2 s/10 s 调度、不删除双有效世代、不压缩撤销保障，也没有以清空工作集掩盖实际分配。

主要生产变更在 `src/large_document.rs`。`src/reading_state.rs` 只为两个现有 worker 添加名称，便于归因。`memory-profile` 是单独诊断特性，普通候选未启用；它的 allocation header、原子计数及 C hook 会改变分配开销，因此不将 profile 候选性能用于最终前后比较。

## 测量口径与排除的试跑

每个尺寸和恢复选择启动独立原生进程；相同 ASCII 夹具为目标尺寸减 256 B，以容纳后续输入。场景为打开默认视图、Write、120 次已提交单字符剪贴板粘贴、滚动、Read/Write 切换、等待检查点、手动保存和稳定观察。没有强行终止 IME 组合。记录阶段 WorkingSet64、PrivateMemorySize64、Windows 原生 PeakWorkingSet64、每次输入的 update 时间、worker 耗时、实际观察到的 writers/pending。单独采样进程 sleep 50 ms，实际间隔包含 JSON/进程读取开销，汇总保留其分布。未采中的短 worker 不表述为没有备份。

最终测试在 `ProcessStartInfo.Environment` 明确移除 SEARCH/RICH/STRUCTURE_REPORT，实际保留 UI、performance、一次启动 renderer 三个元数据报告；每组的 `launch-environment.json` 记录实际 key 存在性，renderer 指向该组自己的隔离目录。不开启全文搜索诊断，也不在诊断文件输出恢复正文。最终前后测量期间无并发编译或长测试。

早期 pilot 的 `.NET SetEnvironmentVariable(null)` 没有充分保证实际子环境移除空字符串报告路径；发现了本任务生成的搜索诊断临时文件。已只对确认属于本任务夹具的两份文件记录 metadata/hash 后清理（`pilot-diagnostic-failure.json`），原始用户文件未动。pilot 659/1722 MiB 等值不作最终降幅。第一次 profile 候选 Windows TLS 初始化递归导致 stack overflow，修正分配器小分配路径后继续，失败记录保留。阶段标记文件的短暂读取占用亦已在脚本有限重试，不改业务代码。

## 本次候选、结果与边界

普通新候选为 `artifacts/memory-2026-10-26/final-r3/mdcard.exe`，SHA256 `5715ebe7ee06370d16ece534a6549ac0e8a5b295b157666f783f9ca9b40d59fc`；故障候选 SHA256 `5ab95ed7b6cdc5683a52255ea084c1c0fef4a95171ab104f57aa8b5f14cf9194`。HEAD 为 `dd168ff02e0791e8e9260c121c004cde3fac4800`，当前未提交源码指纹 `009119d4138e0ada2fdb189bafbb019bd44dcc2ad4ee3ce35361e6b99a8ddd98`，完整清单在同目录 `candidate-manifest.json`。原候选仍为 C84C…5D64，未覆写；普通新候选不启用 `memory-profile`。

本次实现与验收已完成。机器索引：[final-summary.json](../evidence/memory/2026-10-26/final-summary.json)。主代理独立终审逐项核对 117 个源码输入、候选与原始测量、12 份保存全文及全部原生结果：[master-final-audit.json](../evidence/memory/2026-10-26/master-final-audit.json)；另有[独立保存全文核验](../evidence/memory/2026-10-26/master-saved-content-audit.json)。

测量平台为 Windows 11 家庭版中文版 x64 10.0.26100、125% DPI，OpenGL / NVIDIA GeForce RTX 4060 Laptop GPU / NVIDIA 596.08；各组 `renderer.json` 记录实际值。构建与 Linux 回归为 Debian 12 x86_64 容器 `mdcard-dev`，Rust/Cargo 1.92.0。

以下两份完整原生运行使用同一脚本和输入：`before-final-clean/result.json`、`after-final-clean/result.json`，相对于 `evidence/memory/2026-10-26/`。`comparison.json` 保留全部阶段、样本和分布。汇总程序逐字验证了 12 份手动保存正文均等于原夹具、marker 和 120 次输入，不通过少输入或跳过保存降低内存。

| 稿件 / 恢复 | 原生 WS 峰：旧→新 MiB | 采样 PrivateBytes 峰：旧→新 MiB | WS 峰下降 |
| --- | ---: | ---: | ---: |
| 1 MiB / Off | 306.51→306.39 | 279.19→280.33 | 0.04% |
| 1 MiB / On | 294.10→304.97 | 267.12→276.71 | -3.70% |
| 8 MiB / Off | 965.65→435.13 | 981.11→411.05 | 54.94% |
| 8 MiB / On | 958.77→425.50 | 975.70→424.57 | 55.62% |
| 16 MiB / Off | 1666.75→614.62 | 1750.94→605.73 | 63.12% |
| 16 MiB / On | 1695.99→657.26 | 1778.80→668.91 | 61.25% |

1 MiB 仍采用全篇 Tree-sitter，本轮没有内存改善；单轮测量不支持对其小幅波动作确定性解释。16 MiB 开启恢复仍有约 657 MiB 进程峰，不能称低内存。PrivateBytes 是实际采样峰，未宣称捕获到采样间隙中的绝对提交峰；WS 另有操作系统原生峰。采样 sleep 50 ms，三组 On 实际间隔中位数均 63 ms，P95 为 71/78/90 ms，最大 111/119/164 ms。

以下是恢复 On 的完整输入样本和实际提交 worker 时间，均为 n / median / P95 / max，单位 ms；原始 Off 与环形缓冲区保留的 update 样本亦保留在 JSON，没有按耗时筛掉慢帧。

| 尺寸 | 输入 update 旧→新 | checkpoint worker 旧→新 |
| --- | --- | --- |
| 1 MiB | 124 / 2.505 / 3.516 / 4.065 → 124 / 2.111 / 2.380 / 2.577 | 3 / 25.601 / 27.035 / 27.035 → 3 / 20.506 / 24.459 / 24.459 |
| 8 MiB | 124 / 13.752 / 16.994 / 22.386 → 124 / 12.006 / 14.268 / 17.793 | 3 / 94.725 / 118.947 / 118.947 → 3 / 90.861 / 105.771 / 105.771 |
| 16 MiB | 124 / 22.090 / 26.079 / 28.484 → 124 / 22.815 / 26.928 / 32.410 | 3 / 159.508 / 195.875 / 195.875 → 3 / 193.941 / 355.288 / 355.288 |

16 MiB 输入时间约持平、最大值略高，worker 本轮更慢；没有宣称全面加速或没有代价。worker 仅 3 个提交，P95 等于 max，不能据此推出跨机器稳定分位数。2 s/10 s 是调度目标，实际提交还包含 worker 的读取、序列化和同步，不能将其写为提交 SLA。三组 On 都实际观察到最多 1 个 writer，新版 pending version 的观察上限为 0/1/1；实现上限仍为两个 pane worker、零排队写入快照、每 pane 一份最新版本元数据，未放宽队列。

每组输入数组保留完整的 124 个观测输入帧；all-update 是最后 2048 个样本的有界环，并非完整启动历史，没有按耗时过滤。新版 On 的 all-update median/P95/max 为 1 MiB：0.446/0.996/6.125 ms，8 MiB：0.464/2.812/17.793 ms，16 MiB：0.485/7.072/32.410 ms；完整新旧数组在原始报告中。

## 剩余内存的诊断证据

另用独立 profile 候选跑相同 clean 16-On 场景，普通性能表不引用 profile 的耗时或物理内存。`profile-before-clean/16-on/stages.json` 的 continuous-40 同一 UI 报告中 C core requested live 1042.75 MiB、Rust requested live 281.35 MiB：C 占两类已计数请求约 78.75%，这是所计数请求的占比，不是全部进程物理内存的占比。原生/GPU、allocator metadata 和其它 C 库未完全计数。

新 `profile-after-clean/16-on/stages.json` 中 C core live/peak 仅约 0.0028 MiB；Rust requested 历史峰 549.05 MiB，saved-settled live 330.89 MiB。同一稳定阶段 Rust 分组 live 为 other-or-small 231.89 MiB、Markdown parser 96.98 MiB、reading index 2.00 MiB、reading precision 0.01 MiB，恢复/保存 worker 0。独立历史分组峰约 parser 248.01、reading index 26.02、precision 25.79、recovery 96.19、document save 32.00 MiB；这些峰发生时间不同，不相加成总峰。恢复 worker 的 96 MiB 说明序列化/有效世代读取有成本，也说明它未解释优化前约 1 GiB 的 C 树存活量；从 UI 移入 worker 的源快照仍按初始分配来源计入其它组。

剩余 Rust aggregate 为实测。按代码，正文/保存基线/原磁盘字节、Rope、文档级撤销、rich 原文与预览、source map、分页转换缓存、字体/galley，以及恢复/保存瞬时字符串均参与内存；本次没有把每个容器单独标记，因此对这些细分占比只作来源解释，不伪造精确百分比。工作集还含已释放但 allocator/系统尚保留的页和图形库资源，不能用 requested heap 推算完整 WS，或将全部剩余量归于恢复。

诊断候选哈希：before `profile-before3/mdcard.exe`、after `profile-r3/mdcard.exe`，位于 `artifacts/memory-2026-10-26/`；before SHA256 `047b74c529f2173b1bb9f02880e239b7206fd7ad296b51d3963986e610703b83`，after SHA256 `c8b6ff148144a3825649d2eca861afd8edc77ea22b2e081a409effd5e03c5325`，均在总索引记录。当前源码的单独 Parser 释放试验再次得到 546.18→0.25→0 MiB（`tree-parser16-current.json`），独立验证 ownership 归因。

## 本次可靠性与检查

最终根测试 `memory-root-r3-confirm.log`：281 passed / 0 failed / 6 ignored；两平台 all-targets/all-features Clippy、fmt、Linux release 与 Windows 普通/故障构建均退出 0。Windows 专项使用实际 `Start-Process -Wait` 与退出码：恢复 30/0/2、大文档 16/0，测试程序 SHA `8c589fc70df8f2080a6c5f3a1dc34584f4416855c0392c9327bc4ef7b04fb4a5`，结果见 `memory-windows-*-r3-result.json`。

完整 `check.py --desktop --acceptance` 在本次较早检查链通过所有 7 个独立 crate（`memory-check-final.log`）；最后新增的 reset 隔离和测试模型修正随后由最新根测试、Windows 专项及最终原生用例验证，不把旧检查链描述成最后源码的重新构建。

本次保留两次实际失败：root 的“drop Editor 后立即 claim”把尚持有 session Arc 的同进程 worker 当成退出，claim 正确拒绝；修测试为有界等待实际 lease 释放，另用受控 channel 证明已发完成信号但仍持 lease 时必须拒领，真正释放后才能领取。reset 回归的第一版期望少计一个原块末尾换行，已改为同内容 fresh Worker 输出及源范围对照，生产换行未改。新增 reset/generation 分支清空旧边界和 Rope，阻止合并到 reset 的单 edit 使用上一稿范围。失败日志和通过日志均保留。

三套原生使用同一普通候选 5715…59FC；唯一表格注入 case 使用专用 faults 候选，未将受控失败声称为自然发生。

| 套件 | verification ID | 本次结果 |
| --- | --- | --- |
| Large | d92ae50a-35a4-47b3-8db4-ff1e8bb29e30 | 3 个记录步骤；实际 16 MiB 两次强杀、界面恢复、UTF-8 长度/digest/可见全文复制、原文件全字节/hash 不变 |
| Core | f9ac6798-e7cd-4293-9527-10e5896f823e | 14 个记录步骤；主副稿/未命名/空正文、外部变化、取消/稍后/删除/放弃、设置持久化、真实存储失败与重试、最终干净重启 |
| Drafts | 59d09337-309d-474d-b6b4-d224be0716b1 | 16 个记录步骤、7 个恢复 case；有效/无效公式和图表、真实 rich 冲突、正常表格、受控表格 patch 失败；草稿 Copy、正文一致、正常表格手动/自动保存及干净重启无误报 |

原生强杀只针对脚本自己启动的 PID。补充 `verify-recovery-windows.ps1 -Suite Large` 覆盖实际 16 MiB 编辑、最新检查点、强杀、启动界面恢复、可见源码复制与长度/digest、原文件 SHA256，再编辑、再次强杀及再次恢复。Core/Drafts 回归继续验证主副稿、独立草稿和生命周期。

已补真实 worker 的混合 Markdown 跨 1 MiB 增长、缩小、后续局部编辑，以及小段巨型粘贴不能绕过范围上限的回归。语料包括直接 `$...$`、`$$...$$`、围栏公式、HTML、脚注、全局 reference、Unicode、长代码和引用嵌套表格，并核对线性 source mapping 的原文字节和代码分页原文。此矩阵不等于对全部 Markdown 组合的数学等价证明。

未验证系统崩溃、物理掉电、其它操作系统的本次原生内存、其它 DPI/驱动/完整 IME 实现；工作集与 PrivateBytes 包含应用其它模块，不能把所有残余内存都归于恢复。原始输入延迟为应用 update wall time，不包含系统输入队列、GPU 呈现或显示器 scanout。

## 复跑

```powershell
# 原候选与新候选分别测量；使用全新输出目录，独占桌面。
pwsh -NoProfile -File scripts/verify-memory-windows.ps1 -Exe artifacts/recovery-2026-10-26/final-r4/mdcard.exe -OutputDirectory evidence/memory/new-before
pwsh -NoProfile -File scripts/verify-memory-windows.ps1 -Exe artifacts/memory-2026-10-26/final-r3/mdcard.exe -OutputDirectory evidence/memory/new-after
python scripts/summarize-memory.py evidence/memory/new-before evidence/memory/new-after --output evidence/memory/new-comparison.json
pwsh -NoProfile -File scripts/verify-recovery-windows.ps1 -Suite Large -Exe artifacts/memory-2026-10-26/final-r3/mdcard.exe
```

诊断候选使用 `cargo build --locked --release --target x86_64-pc-windows-gnu --features memory-profile`，务必复制到另一个全新目录；普通前后对比不启用该特性。独立 C 树计数入口仅限单独进程：设置 `MDCARD_TREE_MEMORY_REPORT=<isolated-output.json>`，执行 `cargo test --locked --release --features memory-profile tree_memory_profile -- --ignored --test-threads=1`。该测试在创建任何 Parser 前安装全局 allocator hook，不能与其它解析测试并发运行。
