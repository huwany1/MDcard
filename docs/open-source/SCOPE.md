# 公开范围与历史策略

本次以本地源码基线 `a73f3676b0129126fd71ea340355a4aa562adc86` 为输入，以既有公开 main `2e8279f42db82c999f020199bcc16731ede9559f` 为父系建立 `codex/public-source-20261024`，再通过可追溯合并进入 main。不把本地所有分支、标签或开发历史推到远程。

本地开发历史包含一次性部署配置、原始安装状态、日志及截图；清洁导入避免把这些历史 blob 间接上传。没有重写或删除本地历史；修改前已在维护环境创建完整 `git bundle --all` 备份，原源码 checkout 保留。公开仓库仍保留原先 4 个提交和发行标签。逐文件本地来源映射在[import-inventory.json](import-inventory.json)：638 个原 tracked 路径，473 导入，165 排除。此表记录基线 SHA256；许可证/SPDX、开发入口、ELK 隔离及文档修订使部分公开文件与基线不同，最终以公开 commit tree 为准。

## 纳入

| 内容 | 原因 |
| --- | --- |
| src、build.rs、Cargo.toml/Cargo.lock、vendor | 桌面程序完整自有源码、锁定依赖、带原许可的第三方修改源码 |
| assets、locales、examples | 编译嵌入字体、图标、语言资源、演示稿；原始字体/可编辑 SVG/生成工具均保留 |
| renderer 自有 JS、npm lock、构建脚本、来源 JSON、许可 | 本地公式/图表/导出完整入口，可重新取得依赖与运行时 |
| tests 各独立工程与 Cargo.lock、合成 fixtures | 主要安全/行为回归所需输入 |
| scripts、Dockerfile、CI | 主开发入口和历史验收工具；历史工具的额外环境需求明确说明 |
| website 静态源码、lock、公开样稿/截图/导出 | 独立网站可构建；公开演示素材有来源，普通构建不读私有 dist |
| README、CONTRIBUTING、模块/授权/验收文档 | 使用、贡献、已实现边界和历史结论追踪 |
| 既有 RELEASE_NOTES.md、SHA256SUMS.txt | 保持原远程 blob 字节，旧下载资产不受本次许可切换影响 |

## 排除

| 内容 | 排除理由 |
| --- | --- |
| target、测试 target、node_modules、renderer/vendor 生成 bundle、renderer/runtime | 编译缓存、下载依赖和运行时；所需源码/lock/固定来源与构建脚本已提供 |
| dist 与 artifacts 下完整包和重复安装目录 | 大包继续经现有 Releases 分发，不重复放源码 Git |
| 根 evidence 与 website/evidence 原始 QA 文件 | 日志、截图、阅读位置/安装状态、浏览器扩展和机器路径不是构建输入；不整体公开 |
| website/.openai | 维护者托管标识和部署配置，普通静态部署不需要 |
| 网站 6 个一次性发行/部署脚本 | 上传、恢复发行与托管操作不属于开发构建；不把凭据操作当成普通流程 |
| 私人稿件、偏好、环境文件、密钥与部署凭据 | 不在公开清单；实际树与可达历史另行扫描，不能只靠 ignore |

精确排除路径和理由在来源清单中逐项列出。新 .gitignore 仅防止已识别的本地输出重新进入索引，不用于隐藏扫描命中。本次公开扫描直接读取 staged/commit blob，包括原远程祖先；没有按 ignore 跳过实际发布文件。后续新增文件也须重新审核。

原始本地失败与成功证据保留在维护环境。本次只发布摘要及去除敏感值的证据，不把没有公开的历史日志变成有效下载链接。
