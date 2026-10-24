# 全局搜索、Quick Switcher 与正则替换契约

2026-10-22 后续审计消除未变化索引的重复哈希、全篇行号分配和快照重建，并分开记录排队、处理与 UI 轮询耗时。跨完整复检周期的负载对照和最终候选包验收见 [后续台账](BUG_AUDIT_FOLLOWUP_2026-10-22.md)；有限样本不构成任意环境下低于 50ms 的保证。

实施基线：`codex/global-search`，原生 Rust/egui。唯一实现子代理修改模块，主代理独立验收。保留结构编辑、编码安全保存与六语言 UI。

| ID | 冻结验收 |
|---|---|
| GS01 | 后台增量缓存；1600 篇 Markdown，查询无结果、常见词、Unicode、正则；统计引擎、worker 往返和真实 UI 可见 p50/p95/p99/max；热查询 <50 ms，明确初次索引与呈现边界 |
| GS02 | Ctrl/Cmd+P；文件名、路径、首个标题、frontmatter aliases 权重模糊匹配，MRU；上下、Enter、Shift+Enter 双文档分屏；安全创建不存在的相对路径文档 |
| GS03 | 当前文档/工作区；Aa/全词/regex/include/exclude；$1、$2、$<name> 与跨行；分组上下文/行号/高亮/精确跳转；先 diff，逐条/文件/全部 |
| GS04 | 无效 regex 内联；线性时间引擎、模式/文档/结果/输出预算、取消/截止；单文档一次事务 undo/redo |
| GS05 | dirty 内存叠加；替换前验证预览/磁盘；编码/BOM/换行保留；后台持久历史与逐文件原子写，失败补偿；拒绝覆盖外部更新；重启发现历史并回滚；根目录约束/跳过链接 |

全局根目录在打开检索时固定，打开子目录文档不自动缩窄它。文件刷新由后台轮询完成；元数据相同内容变化通过周期完整重读检测。搜索不执行回溯引擎，反向引用与 lookaround 等不支持语法显示内联错误。多行使用 `(?m)`、跨行任意字符使用 `(?s)`。全词只过滤匹配外边界，不插入捕获组；零宽匹配按 Unicode 字符边界前进，重叠匹配按 regex 左优先非重叠语义。

跨文件事务保留本地 `.mdcard-history` 的原始/新字节和 manifest，不宣称文件系统多文件原子性。可恢复快照在第一次修改前持久写入，逐文件原子替换；遇到冲突/失败按已写列表补偿，补偿冲突保留历史供用户恢复。回滚同样校验替换后字节与所有打开缓冲区，拒绝静默覆盖更新。单文档查找替换只修改内存，通过现有 source undo 单次提交；跨文件替换显式保存被选文件的 dirty 内容。

## 使用与可恢复事务

- `Ctrl/Cmd+P`：Quick Switcher。↑/↓ 选择、Enter 打开、Shift+Enter 右侧打开另一文档；无结果时 Enter 新建同名 Markdown（父目录须存在）。文件名、路径、前 512 行中的首个 `# Heading`、frontmatter `alias`/`aliases`（标量、行内数组或列表）参与模糊打分；MRU 在当前会话内保存。
- `Ctrl/Cmd+F` / `Ctrl/Cmd+H`：当前主文档查找替换；`Ctrl/Cmd+Shift+F`：工作区全文检索。结果按文件分组显示匹配上下文和行号，点击后选中原匹配词并滚到目标位置。搜索结果携带源内容签名，内容变化后拒绝沿用旧位置。
- Aa 区分大小写，ab 使用 Unicode 词边界，.* 使用 Rust regex 语法；Include/Exclude 采用相对路径 Glob，以 `;` 分隔。例：Include `**/*.md`，Exclude `archive/**;drafts/**`。当前文档不受隐藏的工作区路径过滤影响。
- `(?m)^(?<animal>cat) (\d+)$` 与 `$<animal>=$2suffix` 可得到 `cat=12suffix`。`$$` 输出字面 `$`，`${name}` 兼容 Rust 形式。`(?s)` 允许 `.` 跨换行；CRLF 严格模式可显式用 `(?mR)`，或在模式中写 `\r?`。替换框支持实际换行。
- “预览变更”展示红色删除/绿色新增及上下文。默认选中全部；也可只替换一条、一文件或所选集合。预览与当前文档身份、内容、编码、BOM、磁盘原字节均校验；过期预览必须重新生成。
- 当前文档替换通过现有 source undo 事务，Ctrl/Cmd+Z 一次撤销，Ctrl/Cmd+Shift+Z 一次重做。工作区替换会保存被选文件的 dirty 内容，包括主窗格和第二窗格。后台提交期间禁止编辑和切换，窗口仍持续绘制并响应系统消息。
- “历史快照”支持跨重启发现并回滚。`.mdcard-history/<id>/` 包含原/新字节及有状态 manifest，记录原保存格式、pending 格式和替换前 dirty 源码。回滚恢复原字节及原内存 dirty 状态；替换后的外部修改或已开文档的新修改会产生冲突错误。历史默认保留，不进入文件树或全文索引。

## 并发、增量与限制

`mdcard-index` 独立扫描和重读文件，通过引用共享的不可变缓存快照发布给 `mdcard-search`。查询线程不等待磁盘扫描。常规刷新约 750ms；每 10 秒完整重读以发现 mtime/大小相同的内容修改。UI 输入在本帧合并后立即发起查询，没有人为 debounce；结果世代隔离过时响应。后台 mutation 的结果始终回传，不使用取消丢弃已完成写入。

总索引预算 128 MiB、100000 文件、500000 条目、刷新预算 15 秒；扫描逐条目检查文件/条目/时间预算和取消，内容读取逐文件检查同一截止时间，单次阻塞 I/O 无法强制中断，因此不承诺严格 15 秒墙钟完成。每文件打开与批量替换编码后上限 16 MiB，匹配/预览最多 10000 条，模式最多 1024 字节、替换模板最多 64 KiB、编译 NFA 最多 1 MiB。搜索超限显示“结果不完整”，替换不会提交一个被截断的预览。文件类型沿用编辑器统一策略，不区分 ASCII 大小写跳过构建/依赖/历史目录、排除符号链接、Windows junction/reparse 点；事务再次检查完整祖先路径与根目录约束。新建使用 `create_new`，不会覆盖已存在文件。

ReDoS 防护不依赖回溯引擎。每次捕获查找前，按**展开后的实际 Thompson NFA 状态数 × 剩余输入字节**扣除工作预算，包含 Unicode 类与计数重复的展开成本；预算由截止时间（默认 500ms）保守换算为 64 state-byte/μs。查找前、每条捕获后、每文件之间检查取消与截止，超额在进入重匹配前拒绝。16MiB 贪婪捕获的独立对抗测试曾暴露仅事后检查延迟 746ms，现以扫描前工作预算修复；八种模式均在毫秒内返回/拒绝。lookaround 和模式内反向引用不在 Rust regex 支持范围，会内联提示语法错误。

事务采用“全量 preflight → 原/新快照持久化 → prepared manifest → 每文件原子写 → committed manifest”。它没有文件系统级多文件原子性；中途失败按已写列表补偿。补偿也失败时保留 `recovery_required`，重启后能继续回滚。快照 manifest 与备份读取限长、拒绝链接与逃逸/重复路径；恢复文档的解码和保存格式在任何写入前验证。

## 验收证据

完整验收记录见 `evidence/search/`。`tests/search-acceptance` 和 `scripts/verify-search-master-windows.ps1` 由主代理独立编写，不依赖实现自报结果。最终执行与二进制哈希以 JSON / 日志为准，下面的测量不能外推为任意硬件、任意大小或任意正则都低于 50ms。

| 验收项 | 实测与入口 |
|---|---|
| GS01 原生引擎 | 独立 1600 篇 / 18,659,070 bytes，Windows/Linux 160 轮持续多次索引刷新与真实修改；`evidence/search/master-review/performance*.json` |
| GS01 真 UI | Linux X11/Mesa，1600 篇 / 13,674,346 bytes，100 轮、32.14 秒，含常见词/无结果/Unicode/正则与增删改、完整字节复检。输入 changed 到实际结果绘制包含 worker、UI poll、呈现：p50 **19.58ms**，p95 **35.74ms**，p99/max **63.10ms**；worker p95 **18.72ms** / max **25.62ms**；`evidence/search/linux-ui-performance/performance.json`。真实 UI 存在一次超过 50ms 的调度尾延迟，未将它混同为引擎耗时或删除；Windows 独立 UI 记录见 `evidence/search/master-review/windows-ui-performance/performance.json` |
| GS01 Windows 真 UI | 同为 1600 篇 / 13,674,346 bytes，100 轮五类 query，每轮断言匹配数，151 秒包含稳定键鼠间隔及增删改/完整复检。输入 changed→绘制结果 **p95 14.21ms / max 14.46ms**，worker max **12.65ms**，engine max **12.52ms**；`evidence/search/master-review/windows-ui-performance/performance.json` |
| GS02/GS03 实际 GUI | Windows、Linux 鼠标键盘验证捕获组 diff、一次撤销重做、按 Glob 跨文件提交、磁盘/内存同步、重启回滚、标题/alias/MRU Quick、键盘/分屏/缺失新建及 Unicode 第 350 行精确选择；`evidence/search/master-review/windows/result.json`、`evidence/search/linux-final/result.json` |
| GS04/GS05 核心 | 根项目全部单测、独立捕获/Unicode/零宽/截止对抗/路径防逃逸/dirty encoding/过期预览/外改拒绝/重启回滚；第 2 文件真实注入写失败、补偿失败与 `recovery_required` 再恢复，回滚中途失败的反向补偿；日志 `evidence/search/core-linux.txt`、`evidence/search/master-review/core-windows.txt` |
| 六语言 | zh-CN、zh-TW、en、ko、ja、ru 的搜索、Quick、内联错误截图与目录完整性/字体测试；`evidence/search/linux-final/*-search-error.png`、`*-quick.png` |

最终检查：根项目单测 **71/71**；主代理独立 harness 连同其引用的原模块单测 Windows **47/47**、Linux **49/49**，均零失败/跳过；`cargo clippy --locked --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过。包含从 Workspace 无效 Glob 切到 Single 的回归，Single 连隐藏过滤器语法解析也会跳过。

复现命令：

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --manifest-path tests/search-acceptance/Cargo.toml --release --locked -- --nocapture
python3 scripts/verify-search-linux.py
python3 scripts/verify-search-ui-performance.py
python3 scripts/package-rich.py --module search --windows-exe artifacts/global-search/MDcard.exe --linux-exe artifacts/global-search/mdcard
```

Windows GUI：`pwsh -File scripts/verify-search-master-windows.ps1 -Exe artifacts/global-search/MDcard.exe`。上述模块历史验收为 Windows 本机和 Debian/X11 软件 OpenGL，未声称已经验证所有 Linux 桌面、Wayland、macOS、DPI 或输入法组合。当时交付的是独立 `dist/MDcard-0.1.0-search-*-x64`，该包现已清理；当前保留的发行包是 `dist/MDcard-0.1.1-windows-x64.zip` 与 `dist/MDcard-0.1.1-linux-x64.tar.gz` 完整包。2026-10-22 的后续缺陷修复和新证据见 [Bug 台账](BUG_AUDIT_2026-10-22.md)，不能用本节历史证据替代新候选包验收。
