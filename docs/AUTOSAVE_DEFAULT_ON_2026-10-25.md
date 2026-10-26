# 自动保存默认开启变更 · 2026-10-25

> 历史记录：2026-10-26 的稿件恢复任务已将当前源码的原文件自动保存默认值改为关闭、独立恢复保护默认开启；本页仅描述当时构建，不能用于当前默认行为。[当前说明](CRASH_RECOVERY.md)。

用户在初版交付后明确要求“自动保存改为默认开启”。本次仅调整默认策略和相应测试、文档、GUI 验证起始步骤；保存、取消、冲突和草稿保护流程不变。

- 全新设置、旧设置缺少 `autosave` 字段、设置读取失败后回退默认设置：自动保存开启。
- 已经保存 `autosave: false`：继续关闭，不被新默认值覆盖；设置开关及其持久化继续有效。
- 未命名文档仍需先手动保存，停止有效编辑 2 秒后写入原文件的规则不变。

[使用说明](AUTOSAVE.md)已更新。[初版默认关闭验收](AUTOSAVE_ACCEPTANCE_2026-10-25.md)保留为历史，并明确标注默认策略已被本次要求取代。旧候选 `artifacts/autosave-2026-10-25/`、旧证据 `evidence/autosave/2026-10-25/` 均未覆盖，不能用其哈希或默认关闭结论描述当前构建。

## 本次验证

最终 Windows 候选 SHA256：`1508C086C0A250959E65C2968E2D882F0E8B346AFF338355A94CF5AEEF6CBDEC`。本轮真实 GUI 完成于 `2026-10-25T23:56:35.4058715+09:00`（Asia/Tokyo），验证 ID：`c61c32fb-560e-44b3-9366-4478687455a2`。

本次独立候选位于 `artifacts/autosave-default-on-2026-10-25/`，本地证据位于 `evidence/autosave-default-on/2026-10-25/`。当前源码、候选 SHA256、GUI 验证身份和四份稿件的实际内容/哈希核对见该目录的 `final-summary.json` 与 `windows/result.json`。所有命令及截图都是本次默认开启变更的新运行。

| 检查 | 本次结果 | 原始日志 |
| --- | --- | --- |
| `python3 scripts/check.py --desktop` | 退出 0：fmt 检查、Linux 全目标 Clippy、根测试、Linux release 全部完成 | `desktop-linux.log` |
| `cargo test --locked`（包含在上项） | **243 passed / 0 failed / 3 ignored** | 同上 |
| `cargo clippy --locked --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | 退出 0 | `clippy-windows.log` |
| `cargo build --locked --release --target x86_64-pc-windows-gnu` | 退出 0；产物只复制至本次独立候选目录 | `build-windows.log` |
| `cargo test --locked --target x86_64-pc-windows-gnu --no-run` | 退出 0 | `test-build-windows.log` |
| 原生 Windows `mdcard-tests.exe autosave:: --nocapture` | **20 passed / 0 failed / 0 ignored**，退出 0 | `autosave-tests-windows.log` |
| 原生 Windows `mdcard-tests.exe preferences:: --nocapture` | **3 passed / 0 failed / 0 ignored**，退出 0 | `preferences-tests-windows.log` |
| `pwsh -STA -File scripts/verify-autosave-windows.ps1` | 退出 0；真实窗口、鼠标、键盘与实际文件断言通过 | `gui-windows.log`、`windows/result.json` |
| `git diff --check` | 退出 0 | `final-summary.json` |

设置迁移测试使用真实临时设置文件，覆盖默认、缺字段、损坏/不可读取文件、先前 `false` 的读取与再次持久化，以及 `true/false` 往返。新增 `default_on_saves_named_edits_and_restored_false_stays_disabled` 用可控时钟确认默认开启会保存具有路径的修改、恢复关闭选择后不会写盘。通用编辑器回归夹具显式关闭自动保存以隔离其测试目的；独立的自动保存测试仍完整覆盖功能行为。

本次没有机械重跑 7 个独立 Rust 工程：它们不引入 `Preferences`，本次默认值及测试夹具修改不影响这些工程；初版运行结果仍仅作为其原有历史证据，未冒充本次新结果。

## Windows GUI 与磁盘内容

环境为 Windows 11 x64、125% DPI、OpenGL / NVIDIA GeForce RTX 4060 Laptop GPU，英文界面。使用更新后的可复跑脚本和本次独立候选，验证以下顺序：

1. 设置缺少新字段时自动开启，未点击开启开关就编辑并自动落盘：`01-default-on.png`、`01a-default-on-saved.png`。
2. 主动关闭并实际持久化 `false`，重启后仍关闭；再次编辑超过等待时间，文件哈希不变而内存保持未保存：`01b-explicit-off.png`、`01c-explicit-off-after-restart.png`、`02-disabled-unsaved.png`。
3. 再次开启后保存已有修改，重启恢复开启，连续输入合并、空闲时自动落盘：`03-explicit-on.png` 至 `05-restored-after-restart.png`。
4. 原主副文档独立保存、副文档失败不阻塞主文档、主副重试按钮、未命名首次手动保存、原生退出保护、只读保护、外部修改冲突与另存为保留两版本：`06-main-and-secondary-saved.png` 至 `14-save-as-preserves-both.png`，包括 `06b/06c`。

截图都位于新证据目录的 `windows/` 下。结果文件记录最终候选哈希、时间、操作断言及四份实际文件内容/哈希；交付汇总另行读取磁盘并核对其一致性。旧默认关闭候选和结果保留在原目录。

## 范围

未提交、未发布、未安装或覆盖正式包。Windows 构建使用 Rust 1.92.0 / MinGW GNU 目标，测试在真实 Windows 上运行，未验证 MSVC。Linux 完成构建和测试，没有 Linux GUI 验证；未测其他 DPI/驱动、macOS、Windows 10、真实 CJK 输入法、SMB/断网盘、掉电/崩溃及长期压力。本次未重跑全部真实渲染器/九种导出/网站浏览器矩阵。

根测试默认跳过的 3 项仍为导出取消交接争用探针、搜索争用性能计时、432 项无头语言/主题/缩放矩阵；未将这些项目计作通过。
