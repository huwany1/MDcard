# MDcard 文档入口

当前 Windows 正式版本为 **Icon Suite R3**，Linux 为 **R29**。运行程序、下载包与使用方法以[项目 README](../README.md)及[既有 Releases](https://github.com/huwany1/MDcard/releases/tag/v0.1.1-20261024)为准。源码仓库不含本地 dist/latest.json。历史报告中的版本、路径和测试结果仅代表其采集时状态。

| 阅读目的 | 文档 |
| --- | --- |
| 从公开源码构建、运行和贡献 | [开发入口](DEVELOPMENT.md)、[贡献指南](../CONTRIBUTING.md) |
| 当前开源范围、许可和验证 | [范围](open-source/SCOPE.md)、[授权](LICENSING.md)、[第三方来源](THIRD_PARTY.md)、[开源验收](open-source/ACCEPTANCE.md) |
| 当前 Windows 图标、桌面入口与真实显示验收 | [图标接入合同](ICON_INTEGRATION.md)、[完整图标验收](ICON_INTEGRATION_ACCEPTANCE.md) |
| 阅读位置与精度支持范围 | [阅读状态](READING_STATE.md)、[精度合同](READING_PRECISION.md)、[38 项精度验收](READING_PRECISION_ACCEPTANCE.md)、[R29 阅读精度交付](READING_PRECISION_DELIVERY.md) |
| 搜索与跨文件替换 | [全局搜索](GLOBAL_SEARCH.md) |
| 结构编辑与源码同步 | [表格与块编辑](STRUCTURAL_EDITING.md) |
| 公式、图表与导出 | [数学和图表](MATH_DIAGRAMS.md)、[多格式导出](LOSSLESS_EXPORT.md) |
| 大文件架构与性能边界 | [大文件性能](LARGE_DOCUMENT_PERFORMANCE.md) |
| 后续计划与未完成能力 | [成熟度路线图](MATURITY_ROADMAP.md) |
| 本次清理、旧路径去向与保留理由 | [图标交付后清理](RESOURCE_CLEANUP_ICON_SUITE_2026-10-24.md) |
| 历史模块总索引 | [历史验收索引](ACCEPTANCE.md) |

以下文档保留历史设计、独立验收、缺陷根因和边界，不作为当前版本的重复验收声明：

- 阅读进度阶段记录：[首阶段验收](READING_STATE_ACCEPTANCE.md)、[A 实现](READING_PRECISION_IMPLEMENTATION_A.md)、[B 实现](READING_PRECISION_IMPLEMENTATION_B.md)、[A 独立验收](READING_PRECISION_INDEPENDENT_A.md)、[B 独立验收](READING_PRECISION_INDEPENDENT_B.md)、[A 包交付](READING_PRECISION_STAGE_A_PACKAGE.md)。
- 界面与工作区：[工作台设计](WORKBENCH_REDESIGN_2026-10-22.md)、[工作台验收](WORKBENCH_ACCEPTANCE_2026-10-22.md)、[界面调整](UI_POLISH_2026-10-22.md)、[界面验收](UI_POLISH_ACCEPTANCE_2026-10-22.md)、[工作区根目录修复](WORKSPACE_ROOT_FIX_2026-10-22.md)。
- 缺陷与清理：[缺陷审计](BUG_AUDIT_2026-10-22.md)、[缺陷复验](BUG_AUDIT_FOLLOWUP_2026-10-22.md)、[10 月 22 日清理](RESOURCE_CLEANUP_2026-10-22.md)、[R29 发布时清理](RESOURCE_CLEANUP_2026-10-24.md)。
- 发布身份：`releases/` 下的 JSON 保留各版源码提交、归档哈希和证据入口。当前分平台身份见 `dist/latest.json`。

源码合同和唯一历史证据保持原件。候选包、验收解压目录中的重复文档已随无用产物清理；其历史路径和保留元数据见本次清理报告。
