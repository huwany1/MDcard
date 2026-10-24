# MDcard 整套图标接入验收（2026-10-24）

构建：`icon-suite-20261024-r3`。II-01～II-12 全部通过。候选及正式 Windows 运行目录已由主代理独立验收，发布核准绑定 `artifacts/icon-integration-20261024/master/release-clearance.json`；Windows-only 发布元数据已按此不可变核准记录更新。

## 身份与资源

| 对象 | 身份 |
| --- | --- |
| 产品编译提交 | `9fe090ea938a35b9f5d14ea647d7377b65fd5ab4` |
| 完整包源码/文档快照 | `4e5b9783e4150a9f87786d009b6aa38fec80eab6` |
| Windows EXE SHA256 | `32b781a8b0915f96443b5e827281fc4fc395dabf04a0f340d85c048c03f696c4` |
| Windows 测试 PE SHA256 | `c7f983947e642ad730361620a37668de6f587d810c06d8493318c9e95118ea42`（含测试构造字段修复） |
| Windows ZIP SHA256 | `a3b480372171bb1c8b9514e77e7be1c8a54d740970c06f9f6ca8741e82da1258` |
| 完整 payload | 22,257 个文件，加 manifest 本身；全部已由主代理独立解压并逐文件校验 |
| 图标套装 | 12 SVG、27 PNG、3 ICO，原设计字节不变；工具依赖缓存不进入发行包 |

完整接入映射、回退与 HICON 生命周期见 [接入合同](ICON_INTEGRATION.md)。完整包先冻结，最终外部报告绑定不可变归档哈希，避免因嵌入自身最终哈希反复重打包。

`source/` 包含 Cargo、build.rs、Rust 源码、测试、vendored egui_commonmark 及 compiler dep-info 实际引用的 assets/locales 等资源。产品编译输入记录位于 `build/compile-inputs.json`；快照仅在 `#[cfg(test)]` 的 Editor 构造器增加 `native_icons` 字段，原产品文件字节另存于 `build/original-compile-inputs/src/structure_ui.rs`。manifest 记录这个差异及原输入哈希；不将未重编的 EXE 冒充新的产品提交。

## II-01～II-12

| 标准 | 结果 | 证据与边界 |
| --- | --- | --- |
| II-01 完整资源接入 | 通过 | 主代理 `master/package-independent.json`：全部 42 个图标与已验收原设计逐字节一致，完整套装文件分发；映射见接入合同。 |
| II-02 三个版本实际用途 | 通过 | `master/final-r3-run4/result.json`：最终包 Light 黑/Dark 白实际绘制，系统蓝色标识；实际品牌截图与原生窗口证据分开保存。 |
| II-03 主题自动适配 | 通过 | 最终包真实 Light/Dark 与 System 随 Windows 实际 Light/Dark 变化，六次品牌观察，rect保持不变；主题切换前后原稿相同，系统设置恢复。 |
| II-04 尺寸与透明 | 通过 | 最终包真机 DPI120（125%）；原生BIG40/SMALL20，应用内32px光学源绘制为24逻辑px，fallback=false；透明M及固定几何截图已保存。 |
| II-05 EXE 图标 | 通过 | 主代理 PE 枚举确认同哈希 EXE 的七尺寸 payload 与原 PNG 一致；最终包真实 Explorer 中的蓝色 EXE 图标与原始截图已保存。 |
| II-06 窗口/任务栏 | 通过 | `final-r3-run4` 直接运行/退出重开与 `desktop-extracted-r3-run2` 桌面原生双击均通过；任务栏单组MDcard、Alt+Tab蓝色，WM_GETICON40/20。 |
| II-07 桌面快捷方式 | 通过 | `desktop-extracted-r3-run2`、`link-relocated-r3`、`desktop-relocated-r3`：实际桌面原生双击、正确目标/工作目录/内置蓝图标/AppID、幂等单链接、中文空格新位置重建并再次双击均通过。 |
| II-08 构建与完整分发 | 通过 | Rust1.92 GNU Windows release 构建通过，Linux 共享代码检查通过；从随包 source/ 实际 offline/locked Windows target check 通过。 |
| II-09 最终包独立验证 | 通过 | `master/package-independent.json` 安全解压22,257项SHA/size/精确集合通过；`master/final-r3-run4/result.json` 从该含中文空格解压路径实际运行两次，产品哈希匹配。 |
| II-10 真实显示证据 | 通过 | 最终解压包的实际Windows/Explorer/桌面/任务栏/Alt+Tab/Light/Dark/System截图与裁剪均保存；Windows11 26100、2560×1440、DPI120（125%），未扩张为其他DPI。 |
| II-11 原功能无回归 | 通过 | Linux222/0/3，Windows本机213/0/2；最终包中文/emoji原生编辑、保存原稿校验、三视图、退出重开及随包Node的KaTeX/Mermaid两个节点完成且0错误。 |
| II-12 发布与交接 | 通过 | 正式 `dist/windows` 全量运行前后SHA/精确文件集合通过；从正式目录重建的Desktop MDcard已原生双击并核对进程/哈希/任务栏/退出。README、Windows latest、发布记录按不可变release-clearance更新，Linux及R29历史保留。 |

## 已执行检查

- Windows release 构建：`implementation/build-windows-native-optical.log`，退出 0；产品为上表 EXE 哈希。
- Linux `cargo test --locked --release`：222 通过、0 失败、3 忽略，2.62s；`implementation/test-linux.log`。
- Linux `cargo clippy --locked --release --all-targets -- -D warnings` 通过；格式检查通过。
- 主代理 Windows 本机完整测试：213 通过、0 失败、2 忽略，5.14s；实际捕获 20,846 字节 stdout，`master/windows-native-all.log`。
- 主代理 Windows 本机 branding 专项：3 通过，`master/branding-native-tests.log`；不与完整测试相加扩大分母。
- 随包源码树：`cargo check --offline --locked --release --target x86_64-pc-windows-gnu --manifest-path .../source/Cargo.toml --target-dir /work/target/icon-source-check` 通过，30.81s；`implementation/source-r3-check.log`。
- 主代理完整包解压：`master/package-independent.json` 已通过，解压目的地含中文与空格；归档、逐文件哈希与套装完整性通过。

上述路径均位于 `artifacts/icon-integration-20261024/`。实际构建命令、输入、脚本及 source/ 均可审计；不承诺不同工具链下二进制位级一致。

忽略项：Windows/Linux 均未执行 search 的 full-verification/CPU-contention follow-up timing 与 432-case headless layout matrix；Linux另忽略 export cancellation/CPU-contention follow-up probe。它们不加入本轮通过分母。

## 迭代与边界

最早 Linux 测试因测试构造器缺少 `native_icons` 字段失败，日志保留为 `implementation/test-linux-first-failed.log`；修复后完整通过。r1 源码快照缺少相对编译资源，r2 补齐后保留；最终 r3 另保留原始产品编译文件，满足包内独立审计。早检同哈希程序的截图与最终包截图分别存放，不把早检冒充最终包运行。

本轮正式交付目标为 Windows x64；Linux 仅验证共享源的构建/测试，原 R29 Linux 正式包不改变。MSVC 分支、macOS、其他 DPI 与其他桌面环境未验证。

最终包 GUI 独立运行：`master/final-r3-run4/result.json` 已通过，Windows11 build26100，DPI120（125%），主屏2560×1440。场景含 Light 黑、Dark 白、System 随实际 Windows Light/Dark、保存后的 Read 与重开后的 Light；品牌矩形一致，所有场景 fallback=false。真实随包 Node 路径记录在报告中，公式/图表 completed=2/errors=0；同一二进制从最终解压路径实际启动两次。

桌面入口独立验证：`master/desktop-extracted-r3-run2/result.json` 已通过真实原生双击、准确进程路径/哈希、任务栏单组、WM_GETICON40/20、正常退出以及用户偏好原字节不变。迁移目录从同一r3 ZIP安全解压后通过22,257项校验，随后新位置重建快捷方式、再次原生双击均通过；正式入口的通过证据列于下节。

发布前聚合记录：`master/pre-promotion.json` 将9份独立包/GUI/迁移/运行后审计报告逐个绑定哈希，对应II-01～II-11通过。两个解压目录运行后22,257项payload仍精确一致；额外原生阅读回归 `evidence/reading-state/master/icon-suite-r3-native/result.json` 通过A/B/A与重启位置，offset误差均0，原稿字节不变。该文件保留发布前阶段身份；II-12的最终通过依据为下节正式入口证据、发布核准及发布记录。

## 正式入口与发布

正式可执行文件：`dist/windows/MDcard.exe`；完整包：`dist/MDcard-0.1.1-icon-suite-20261024-r3-windows-x64.zip`。EXE与ZIP哈希均为上表冻结值，未重新编译或重打包。正式桌面MDcard快捷方式已指向该长期保留运行目录，用户默认偏好字节保持不变。

主代理正式目录证据：`master/formal-explorer-r3/result.json`、`master/desktop-formal-r3/result.json`、`master/formal-tree-before-gui.json`、`master/formal-tree-after-gui.json`全部通过。实际EXE、桌面、任务栏与最终包Alt+Tab截图已由主代理打开查看，汇总为 `master/proof-gallery.png`。发布核准 `master/release-clearance.json` 固定绑定最终EXE/ZIP和正式目录检查，不再改写；`dist/latest.json`记录其SHA256。

发布时，R29旧Windows运行目录备份在 `dist/.before-icon-suite-20261024-r3/windows`；图标交付后清理已删除其重复展开，保留完整R29回滚ZIP，原SHA仍为 `efabd1e02c64715b55e2c0eedf2e5b5f542f761ee370d121a2c5086889692412`。Linux正式应用SHA仍为 `b438bd9e042366a77c63bcb9f6ccc989414119788959086cec98114f2c6cd9cf`，Linux正式latest记录按原值保留。发布记录为 `docs/releases/2026-10-24-icon-suite-r3.json`，操作记录为 `implementation/release/staged.json` 与 `promotion.json`。

清理后，候选包及两份验收解压目录的历史路径不再存在；R3程序、源树和套装可从正式完整ZIP提取，候选构建的manifest/身份小型元数据另行保留。真实截图、失败日志、原始夹具、独立核准JSON及其原路径均保留。详细删除清单和保留理由见[清理报告](RESOURCE_CLEANUP_ICON_SUITE_2026-10-24.md)。

早先验证过程中的Winit线程事件窗口句柄缓存、桌面UIA定位、仅改注册表未刷新真实OS主题缓存等驱动失败，均保留原始阶段与失败记录。修复的是验证驱动；真实Windows设置控件切换、原生桌面双击和最终路径/哈希提供最终通过证据。没有因驱动失败额外修改冻结产品或伪造截图。
