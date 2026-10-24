# 第三方来源、许可证与构建边界

本项目未将第三方组件改为 AGPL。版本和完整性以 Cargo.lock、各 package-lock.json、字体来源表和运行时固定 SHA256 为准。

| 内容 | 来源与许可 | 保留位置 / 处理 |
| --- | --- | --- |
| Rust crates | Cargo.lock 对应 crates.io / 上游；MIT、Apache-2.0 等各自许可 | 根 THIRD_PARTY_NOTICES.txt 保留许可/版权；Cargo 从锁定注册表源取得依赖 |
| 本地 egui_commonmark 0.22.0 | lampsitter/egui_commonmark；MIT OR Apache-2.0 | vendor/egui_commonmark 中原 MIT / Apache 全文及原元数据保留；本地修改说明见该目录 MDCARD_CHANGES.md |
| Node 渲染依赖 | renderer/package-lock.json 的版本、resolved、integrity | renderer/THIRD_PARTY_NOTICES.txt 含实际依赖许可/版权/NOTICE，由 licenses.mjs 收集 |
| KaTeX / mhchem / Mermaid / DOMPurify / flowchart.js | 各自上游，不能一律称 MIT | mhchem 的 MathJax Apache 改编声明单独保留；flowchart 依赖 eve-raphael 为 Apache-2.0；khroma 实际许可为 MIT |
| elkjs 0.9.3 | kieler/elkjs，EPL-2.0；未确认有 Secondary License 授权 | 上游依赖可下载且保留 EPL 声明；本项目浏览器构建替换 ELK loader，metafile 断言无 elkjs 实现。新构建不支持 ELK 布局，避免假设其与 AGPL 组合兼容 |
| Sharp / libvips | lovell/sharp，Sharp Apache-2.0；libvips LGPL-3.0-or-later；部分预编译包复合许可 | 保留独立动态组件、许可证和来源；不得描述为纯 MIT。修改/分发它们仍需满足 LGPL 的源码、重链接/替换和反向工程等适用义务；上游来源与下载完整性见锁文件/NOTICE |
| Google Fonts Noto Sans SC/TC/JP/KR、Noto Emoji | Google Fonts 官方仓库；SIL OFL 1.1 | assets/font-sources.json 记录来源与 SHA；assets/OFL.txt、各 *-OFL.txt 保留版权和许可 |
| 字体静态实例 | scripts/generate-diagram-font.py / generate-ui-fonts.py；fonttools 4.61.1 | MDcard Diagram Sans / KR、MDcard UI Sans JP / TC 已改家族名，不使用保留名称 Source；保留 Adobe 版权与 OFL，不改为 AGPL |
| 自有图标 | MDcard 原始几何 SVG 与生成脚本 | assets/branding/mdcard-icon；AGPL-3.0-only，图标工具依赖 resvg 采用其自身许可 |
| PlantUML | 明确选择 MIT 构建 1.2026.9，不能套用到其他构建 | renderer/PlantUML-MIT-LICENSE.txt、runtime-sources.json 固定来源和哈希 |
| Node.js | nodejs.org，MIT 与内含第三方声明 | renderer/Node-LICENSE.txt；下载运行时不入源码 |
| Chromium / Java Temurin / Pandoc | Playwright 官方 CDN / Adoptium / jgm；多许可 / GPLv2+Classpath 例外 / GPL | export-runtime-sources.json 固定下载及哈希；原运行时 legal/license 随包保留，互为独立工具，不改写为 AGPL |
| 网站截图和示例 | 项目自有公开演示稿与原 Windows 产品截图 | website/ASSETS.md 记录来源；公开用的 WebP、HTML、PDF保留，原始 QA 状态和截图目录排除 |

新开发者构建不需重新生成字体：原始字体和已改名的静态实例均已公开。重生成需安装 assets/branding/mdcard-icon/tools/requirements.txt 或上述 fonttools；输出须重新核对 name 表和 OFL。字体不是单独售卖的产品。

本次没有重新分发第三方运行时的二进制，也没有替换旧发行包；若制作新的完整包，应连同实际安装的依赖/运行时许可与所需对应源码提供方式一起检查，不能只复制本表。源码公开不等于自动证明任意未来打包组合合法。
