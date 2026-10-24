# MDcard 整套图标接入合同

日期：2026-10-24。基线：`4a1d53c`（R29），设计资源：`bfb00b0`。本轮完整应用构建 ID：`icon-suite-20261024-r3`。默认系统标识为蓝色；黑/白单色在现有标题栏 MDcard 品牌位按实际背景自动选择。窗口布局、文档、撤销、保存、阅读状态与偏好语义保持原合同。

## 资源映射

| 资源 | 用途与消费位置 | 尺寸选择 | 验证 |
| --- | --- | --- | --- |
| `ico/mdcard-color.ico` | `build.rs` 编译到 Windows EXE 的 RT_GROUP_ICON/RT_ICON；桌面快捷方式 `MDcard.exe,0` | 16/24/32/48/64/128/256，由 Windows Shell 选择 | 独立 PE 枚举与真实 Explorer/桌面截图 |
| `png/color/mdcard-*.png` | 全部内嵌于 Rust，256px 提供初始原生窗口回退；Windows 按实时 DPI 从完整 PE 资源选择大小图标；完整套装供后续显示密度消费 | 窗口初始 256，原生 BIG/SMALL 使用 GetSystemMetricsForDpi + LoadImageW；27 张 PNG 均由统一资源选择器读取 | 原生窗口、任务栏、Alt+Tab 和对应 PNG 校验 |
| `png/black/mdcard-*.png` | 标题栏既有 MDcard 品牌位，实际浅色背景 | 24 逻辑 px × pixels-per-point，选择不小于请求的最近尺寸 | 实际绘制诊断及浅色截图 |
| `png/white/mdcard-*.png` | 同一品牌位，实际深色背景 | 同上；随背景亮度自动切换，无用户图标选择项 | 深色/System 实际主题切换截图 |
| 三个 `svg/mdcard-*.svg` | 可编辑矢量母版，受控源码和发行包资源 | 64×64 设计网格 | 原设计哈希不变、完整分发清单 |
| 九个 `svg/optical/*.svg` | 16/24/32px 手工光学校正设计源稿，对应尺寸 PNG 已预渲染并内嵌 | 依尺寸使用相应 PNG，避免重新缩小主稿 | 原稿/PNG 哈希与选取检查 |
| 黑/白 ICO | 完整图标套装中的单色系统资源与交接资源；本轮默认桌面标识使用蓝色 | 每个真实七尺寸条目 | 清单、源资源和打包哈希 |
| `preview/`、`DESIGN.md`、`ACCEPTANCE.md`、`tools/` | 设计交接、离线实际尺寸预览、可复现制作工具 | 不作为应用运行依赖 | 完整套装分发，排除 node_modules/缓存 |

## 运行行为与失败语义

所有运行时 PNG 通过 `include_bytes!` 固定嵌入，不依赖工作目录或外部图标文件。标题栏品牌位使用同一个 24px 布局框；颜色依据实际 `chrome` 背景计算，兼顾 200ms 主题过渡。Light/Dark/System 沿用当前外观逻辑。若未来资源损坏而解码失败，回退到内建同色透明圆角卡片/M 几何；记录实际 `fallback`，文档仍可访问。

`MDCARD_UI_REPORT` 的 `branding` 数据来自实际绘制响应：variant、resource_size、logical_size、background_rgba、rect 和 fallback。该通道只读，不用于改变主题或图标。`appearance`/`system_theme` 区分用户偏好与系统实际主题。

Windows 固定进程 AppUserModelID 为 `MDcard.Desktop`。桌面快捷方式脚本按自身所在目录查找 `MDcard.exe`，设置同一 AppUserModelID，工作目录为包目录，图标为最终 EXE 的内置主资源；可重复创建/修复。移动目录后从新目录重跑脚本。系统标识保持蓝色，不随应用内主题改成黑/白。

## 构建与完整包

使用当前容器工具链 Rust 1.92、GNU Windows 目标和 MinGW windres；不新增 Rust 资源构建依赖。`build.rs` 在 Windows GNU 目标调用 windres；也保留 MSVC rc.exe 分支，但本轮 MSVC 未验证。Linux 跳过 PE 资源编译并验证共享代码。

Windows 新包复用逐文件验证通过的 R29 全量运行时，替换本轮编译程序与受控当前源码/文档，加入整套 42 图标文件与交接/脚本资源。打包前冻结产品提交；manifest 记录产品提交、编译输入 SHA、所有 payload 哈希及新 build ID。历史 R29 归档和 Linux 正式包维持原身份。候选包经主代理独立解压、真实 GUI 验证后才更新 Windows 正式入口与发布指针。

## 验收边界

主代理逐项验收 II-01～II-12；构建、自检、包日志属于实现证据，真实 Windows 显示另行验收。实际 OS、DPI、显示器、截图路径、哈希与提交记录在最终 `ICON_INTEGRATION_ACCEPTANCE.md`。未测试 DPI、macOS、其他 Linux 桌面和 MSVC 不扩张为已验证。

首轮候选 r1 已保留，主代理发现源码快照缺少相对编译资源，本轮 r2 在 `source/` 补齐实际 compiler dep-info 引用的资源并包含测试源。产品二进制仍绑定 `9fe090e`；当前源码快照另修复一个 `#[cfg(test)]` Editor 构造字段，manifest 分别记录产品输入、源码快照和该测试专属差异，不把旧二进制冒充新编译。

r2 补齐可构建源码树后保留。最终 r3 额外在 `build/original-compile-inputs/src/structure_ui.rs` 随包保留产品的原始编译字节，SHA 强制匹配原输入记录，便于脱离 Git 仓库独立核对与 cfg(test) 快照的唯一差异。
