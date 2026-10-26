# 自动保存交付验收 · 2026-10-25

> **历史记录，默认策略已被取代：** 用户随后明确要求“自动保存改为默认开启”。本文默认关闭/显式开启的结论仅描述旧候选与旧证据，不代表当前源码。当前策略、独立候选和本次验证见 [默认开启变更记录](AUTOSAVE_DEFAULT_ON_2026-10-25.md)。原 `artifacts/autosave-2026-10-25/` 与 `evidence/autosave/2026-10-25/` 保留，未覆盖。

本次在 `codex/explicit-autosave-2026-10-25` 实现默认关闭、用户显式开启的自动保存。主副文档具有独立的防抖、保存和失败状态；手动保存也接入同一异步流程。修改未提交、未发布，没有替换正式安装包。使用说明见 [AUTOSAVE.md](AUTOSAVE.md)。

## 实现与安全边界

- 设置新增 `autosave: false`，兼容缺字段和读取失败；六种语言均包含设置说明、状态与错误文案。设置写入失败会保留本次显式开关选择，告知未记住，并停止逐帧重复尝试。
- 2 秒等待依据有效内容修订及编码/BOM 格式变化，通过定时重绘在空闲时推进；滚动、主题与选择不产生保存。未命名文档不会自动调用文件对话框。
- 后台快照携带缓冲区身份、打开代次、原路径、修订和保存格式。完成结果只更新已保存基线，保留内存中新内容、当前格式、阅读身份和编辑历史。旧打开结果、改路径结果不能应用于当前缓冲区。
- 取消准备阶段的任务必须等其物理结束才允许该窗格再创建任务；换文档保留取消确认，每窗格最多一个实际保存任务。手动保存请求以单项保留，取消确认后保存当前最新内容。
- 复用原子写入核心，在同目录临时文件同步后再次检查磁盘字节基线。提交闸门区分可取消准备与不可取消提交；关闭/放弃在不可取消阶段保留保护并说明实际状态。
- “放弃”立即恢复最后已保存的基线，即使后续打开取消或失败，也不会重调度已放弃的内容。保存与工作区替换/回滚使用共同的进程内及系统锁。Windows 命名锁覆盖同一会话的 MDcard 进程；Linux 文件锁覆盖同一用户。其他编辑器仍依赖提交前基线复核。
- 自动保存失败留在对应文档内，暂停该文档并提供重试，不弹出反复错误对话框；另一文档可以继续保存。

## 本次自动化命令

Rust 1.92.0；Linux 检查在本机 Docker 的 Debian 12 构建环境执行，工作区挂载为 `/work`。Windows 候选及测试程序由同一源码通过 MinGW / `x86_64-pc-windows-gnu` 构建，并在真实 Windows 上运行；未使用 Wine，未验证 MSVC 工具链。

| 命令或执行方式 | 本次结果 | 本地原始记录 |
| --- | --- | --- |
| `python3 scripts/check.py --desktop` | 依次完成下列 fmt、Clippy、测试及 Linux release；退出 0 | `evidence/autosave/2026-10-25/desktop-linux-final.log` |
| `cargo fmt --all -- --check` | 通过 | 同上 |
| `cargo clippy --locked --all-targets -- -D warnings` | 通过 | 同上 |
| `cargo test --locked` | **242 passed / 0 failed / 3 ignored** | 同上 |
| `cargo build --locked --release` | Linux release 构建通过 | 同上 |
| `CARGO_TARGET_DIR=/work/target/acceptance python3 scripts/check.py --acceptance` | 7 个独立工程全部完成，**693 passed / 0 failed / 8 ignored** | `acceptance-linux.log` |
| `cargo clippy --locked --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | 通过 | `clippy-windows-final.log` |
| `cargo build --locked --release --target x86_64-pc-windows-gnu` | Windows release 候选构建通过 | `build-windows.log` |
| `cargo test --locked --target x86_64-pc-windows-gnu --no-run` | Windows 测试程序构建通过 | `test-build-windows-final.log` |
| Windows 原生测试程序 `autosave:: --nocapture` | **19 passed / 0 failed / 0 ignored**，退出 0 | `autosave-tests-windows.log`，对应 stderr 为空 |
| Windows 原生测试程序 `preferences:: --nocapture` | **3 passed / 0 failed / 0 ignored**，退出 0 | `preferences-tests-windows.log`，对应 stderr 为空 |
| `pwsh -STA -File scripts/verify-autosave-windows.ps1 -Exe artifacts/autosave-2026-10-25/mdcard.exe` | 真实窗口、鼠标、键盘和文件内容断言通过，退出 0 | `gui-windows-final.log`、`windows/result.json` |
| `git diff --check` | 通过 | `final-summary.json` |

表内未重复目录的日志都位于 `evidence/autosave/2026-10-25/`。这些是本次生成且被 Git 忽略的本地验证产物；复跑入口脚本随源码交付。原生 Windows 测试使用 `Start-Process -Wait -WindowStyle Hidden` 及独立标准输出/错误重定向，取得实际退出码。Windows 测试中的只读属性恢复仅对 Windows 使用 `set_readonly(false)`；该测试窄范围允许 Clippy 的跨编译 Unix 权限提示，Unix 分支显式使用 `0600`。

独立工程通过数依次为：performance 125；reading 118；reading-precision 90（33 + 23 + 34）；reading-precision-implementation 104；reading-store 109；rich 24；search 123。独立检查运行时的生产代码与交付实现一致，最后增加的 rich 草稿测试辅助函数仅在 `cfg(test)` 编译，随后由主工程和 Windows 原生定向测试再次验证。

## C01–C12 对应证据

调度用例显式传入 `Instant`，异步顺序用可控结果通道/提交闸门验证；不依赖真实等待 2 秒来判定调度。涉及磁盘的 worker 用例仅等待任务完成。跨进程用例让两个原生进程先同时读取同一基线，再同时尝试写入，确认一个成功、另一个报告冲突。

| 标准 | 实现及本次验证 |
| --- | --- |
| C01 默认关闭 | `autosave_defaults_migration_failures_and_explicit_choices` 检查默认、缺字段、无文件、损坏/不可读设置及显式选择；`disabled_untitled_clean_and_non_edit_activity_never_schedule` 检查关闭/无修改不调度；GUI 01–02 核对编辑后原文件哈希未变。 |
| C02 设置及持久化 | 设置读写往返、旧版迁移和六语言目录完整性测试通过；`failed_preference_write_keeps_explicit_disable_and_does_not_retry_each_frame` 确认关闭持久化失败不会重新开启；GUI 03、05 验证真实开关、设置文件及重启恢复。 |
| C03 防抖与空闲推进 | `two_second_debounce_coalesces_and_format_alone_is_an_edit` 精确检查 1999/3998/3999 ms；GUI 连续输入未提前落盘，随后不操作鼠标/键盘也自动保存已有及最新修改。 |
| C04 主副与入口 | `main_and_secondary_save_independently_and_keep_bom_crlf`、格式单独变化及 A/B 用例通过；现有源码、撤销重做、表格/任务节点、当前文档替换回归通过。GUI 06 核对主副两份实际文件。统一调度观察文档修订和保存格式。 |
| C05 首次保存与草稿 | 未命名及首次手动保存用例通过；`valid_rich_draft_must_sync_before_autosave_and_save_keeps_floating_focus` 在真实 eframe 更新中先阻止未同步草稿，再同步、落盘并保留浮动编辑器焦点；IME 阻塞、旧草稿冲突、公式保存/导航保护测试通过。GUI 07–09 验证不弹另存为、首次手动选路径及后续自动保存。真实输入法见未验证范围。 |
| C06 顺序与新编辑 | 可控 A→B、保存格式变化、A 期间撤销回旧基线、手动另存为期间产生 B、旧代次/不同缓冲区/不同路径结果均通过；只接受已保存基线。取消确认的写锁用例连续 32 次重开并保留最新手动请求，未新增在途任务。 |
| C07 生命周期 | `confirmed_discard_is_final_even_when_opening_the_replacement_fails` 真正执行 request→确认放弃→失败打开；取消准备任务后真实重命名旧路径不被重建；副文档关闭保护及 Save 后 Cancel 不误关文档；GUI 原生 X 保护、另存为保留两个版本。 |
| C08 保存安全 | 编码/BOM/CRLF fixtures、不可编码、外部修改、删除、只读、缺失目录/失败写入等测试通过；两个独立进程同一旧基线写入只允许一个成功。GUI 10、13、14 检查只读原字节不变、外部版本未被覆盖、另存为保留内存版本。 |
| C09 失败与恢复 | 外部修改后保持 dirty、禁止自动重试、修复原因后手动重试恢复，以及失败不阻塞另一窗格的用例通过；GUI 06b/06c、10/12 实际点击主副“重试保存”，并检查磁盘内容。 |
| C10 原交互与事务 | 主工程与独立搜索/替换/回滚、导出进程、阅读恢复、结构编辑及撤销回归通过；保存与替换/回滚提交共用系统锁，事务开始前协调保存、结束后再检查当前状态；可控“看似 clean、提交尚未确认”的原生 Close 事件仍被取消；GUI 08、11 验证退出保护。未做完整真实导出格式矩阵。 |
| C11 可理解状态 | 主副状态在各自标题下方、编辑器上方；真实截图呈现默认关闭、等待首次手动保存、已保存、失败原因和可点击重试；dirty 与保存基线测试通过。没有在界面显示内部代次/任务编号，也没有自动保存成功弹窗。 |
| C12 本次交付 | 本表命令、实际 Windows GUI 文件断言、候选哈希、截图、使用说明及未验证范围均已记录；未以历史发行或历史通过代替本次证据。 |

## Windows GUI 证据

环境：Windows 11 x64、125% DPI、OpenGL / NVIDIA GeForce RTX 4060 Laptop GPU、驱动 596.08，英文界面。最终候选为 `artifacts/autosave-2026-10-25/mdcard.exe`；其 SHA256、验证时间、操作断言及四个稿件的最终内容与哈希见 `windows/result.json`，并在 `final-summary.json` 中再次核对候选身份。

| 截图 | 展示及文件检查 |
| --- | --- |
| `windows/01-default-off.png`、`02-disabled-unsaved.png` | 默认关闭；源码已有编辑但原文件字节哈希未改变。 |
| `03-explicit-on.png`、`04-idle-auto-saved.png`、`05-restored-after-restart.png` | 明示原文件写入；主动开启、空闲落盘及重启恢复。 |
| `06-main-and-secondary-saved.png` | 两个文档分别保存；各自文件存在实际编辑内容。 |
| `06b-secondary-failure-main-saved.png`、`06c-secondary-retry-saved.png` | 副文档失败和 Retry 位于可见区域；主文档仍保存；点击重试后副文档恢复。 |
| `07-untitled-waits-for-manual-save.png`、`08-native-exit-protection.png`、`09-after-first-manual-save.png` | 未命名只在内存、无自动路径/对话框；原生 X 提示保护；自行首次保存后自动落盘。 |
| `10-readonly-failure.png`、`11-failed-save-exit-protection.png`、`12-recovered.png` | 只读失败、保留 dirty 与退出保护；点击重试后写入当前内存内容。 |
| `13-external-conflict.png`、`14-save-as-preserves-both.png` | 原文件保留 `EXTERNAL VERSION`；另存文件包含内存修改，两者实际内容已检查。 |

初始主文档的 UTF-8 BOM 和原有 CRLF 段在 GUI 编辑保存后仍存在；新粘贴内容的换行由现有编辑器生成，未将混合换行稿件误称为全文件 CRLF。完整格式保留另由原始字节 fixtures 验证。所有测试窗口在脚本结束时关闭，测试设置与阅读状态均使用隔离路径。

## 未验证范围

- 主工程 3 个默认 ignored：CPU 争用下导出取消交接探针、搜索完整字节验证后的性能计时、432 项无头语言/主题/缩放矩阵；本次没有将其记为通过。
- 独立工程 8 个默认 ignored：reading 的 4 项需独占环境变量的串行持久化交错、reading-store 的 1 项容量/清除交错、search 的 3 项显式 release 性能/对抗超时检查；未运行这些显式专项。未测完整 Windows 原生主工程全量测试，原生只运行表内定向 22 项。
- 未做 Linux GUI、macOS、Windows 10、其他 DPI/驱动或多 Windows 登录会话验证；没有 MSVC 或 Rust 1.88 复验。Linux release 构建不等于 Linux GUI 验收。
- 未做真实中文/日文输入法的组合输入人工验证；本次有组合输入状态阻塞及草稿同步自动化验证。未做真实 SMB/断网盘、硬件掉电/崩溃或长期压力测试。
- 未重跑全部真实公式渲染器、九种导出格式或网站浏览器矩阵；本次没有修改 renderer/网站。没有崩溃恢复或任意外部程序并发写入之间的事务承诺。
- 全部候选写入独立 `artifacts/autosave-2026-10-25/`，测试写入 `evidence/autosave/2026-10-25/`；未运行发布、上传、正式包替换或安装操作。
