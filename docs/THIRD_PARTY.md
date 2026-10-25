# 第三方源码、资产与许可例外清单

本文件集中说明不属于 MDcard 自有内容的源码、资产、嵌入材料及其许可例外。根目录 `COPYRIGHT` 的 AGPL-3.0-only 声明仅覆盖本项目有权授权的部分；下列第三方内容继续采用原许可，原版权和完整许可文本保留。MDcard 对上游的自有代码修改按 AGPL-3.0-only 提供，字体派生和子集继续遵守 OFL。

版本和完整性以 `Cargo.lock`、各 `package-lock.json`、`assets/font-sources.json`、`renderer/runtime-sources.json` 和 `renderer/export-runtime-sources.json` 为准。本清单说明来源与例外，不取代各组件的 LICENSE / OFL / NOTICE 原文。

| 内容 | 来源与许可 | 保留位置 / 处理 |
| --- | --- | --- |
| Rust crates | Cargo.lock 对应 crates.io / 上游；MIT、Apache-2.0 等各自许可 | 根 THIRD_PARTY_NOTICES.txt 保留许可/版权；Cargo 从锁定注册表源取得依赖 |
| 本地 egui_commonmark 0.22.0 | [lampsitter/egui_commonmark](https://github.com/lampsitter/egui_commonmark)；上游原部分 MIT OR Apache-2.0 | `vendor/egui_commonmark/LICENSE-MIT`、`LICENSE-APACHE` 保留原文；MDcard 自有新增和修改采用 AGPL-3.0-only，完整正文见 `LICENSE-AGPL`。修改版包元数据为 `(MIT OR Apache-2.0) AND AGPL-3.0-only`；边界见 `MDCARD_CHANGES.md`、具体补丁说明见 `PATCHES.txt`；应用包同时携带原文和修改说明 |
| Node 渲染依赖 | renderer/package-lock.json 的版本、resolved、integrity | renderer/THIRD_PARTY_NOTICES.txt 含实际依赖许可/版权/NOTICE，由 licenses.mjs 收集 |
| KaTeX / mhchem / Mermaid / DOMPurify / flowchart.js | 各自上游，不能一律称 MIT | mhchem 的 MathJax Apache 改编声明单独保留；flowchart 依赖 eve-raphael 为 Apache-2.0；khroma 实际许可为 MIT |
| elkjs 0.9.3 | kieler/elkjs，EPL-2.0；未确认有 Secondary License 授权 | 上游依赖可下载且保留 EPL 声明；本项目浏览器构建替换 ELK loader，metafile 断言无 elkjs 实现。新构建不支持 ELK 布局，避免假设其与 AGPL 组合兼容 |
| Sharp / libvips | lovell/sharp，Sharp Apache-2.0；libvips LGPL-3.0-or-later；部分预编译包复合许可 | 保留独立动态组件、许可证和来源；不得描述为纯 MIT。修改/分发它们仍需满足 LGPL 的源码、重链接/替换和反向工程等适用义务；上游来源与下载完整性见锁文件/NOTICE |
| Google Fonts Noto Sans SC/TC/JP/KR、Noto Emoji | Google Fonts 官方仓库；SIL OFL 1.1 | assets/font-sources.json 记录来源与 SHA；assets/OFL.txt、各 *-OFL.txt 保留版权和许可 |
| 字体静态实例 | scripts/generate-diagram-font.py / generate-ui-fonts.py；fonttools 4.61.1 | MDcard Diagram Sans / KR、MDcard UI Sans JP / TC 已改家族名，不使用保留名称 Source；保留 Adobe 版权与 OFL，不改为 AGPL |
| 图标构建工具的依赖 | [resvg-js](https://github.com/thx/resvg-js)，`@resvg/resvg-js` 2.6.2，MPL-2.0；[Pillow](https://github.com/python-pillow/Pillow/blob/main/LICENSE) 13.0.0，MIT-CMU（安装元数据标识），见 `assets/branding/mdcard-icon/tools/requirements.txt` | 图标和自有生成脚本为 AGPL-3.0-only；外部工具及其原许可不被图标授权覆盖，版本/来源/完整性见同目录 npm lock 和 requirements；工具本身不作为自有资产重新授权 |
| 字体生成工具 | [fonttools](https://github.com/fonttools/fonttools)，MIT；历史生成版本 4.61.1 | 用于生成/子集化 OFL 字体，工具不作为 MDcard 自有源码重新授权；字体的实际生成器与哈希见 `assets/font-sources.json` |
| PlantUML | 明确选择 MIT 构建 1.2026.9，不能套用到其他构建 | renderer/PlantUML-MIT-LICENSE.txt、runtime-sources.json 固定来源和哈希 |
| Node.js | nodejs.org，MIT 与内含第三方声明 | renderer/Node-LICENSE.txt；下载运行时不入源码 |
| Chromium / Java Temurin / Pandoc | Playwright 官方 CDN / Adoptium / jgm；多许可 / GPLv2+Classpath 例外 / GPL | export-runtime-sources.json 固定下载及哈希；原运行时 legal/license 随包保留，互为独立工具，不改写为 AGPL |
| 网站截图和示例 | 项目自有公开演示稿与原 Windows 产品截图 | website/ASSETS.md 记录来源；公开用的 WebP、HTML、PDF保留，原始 QA 状态和截图目录排除 |
| 网站导出样例内嵌字体 | `website/src/assets/exports/manuscript.html`、`.pdf`、`manuscript-html.zip` 含导出引擎生成的字体/字体子集；来自上述 Noto / MDcard Diagram Sans 字体 | 样稿和自有排版为 AGPL-3.0-only，嵌入字体及其派生仍为 OFL-1.1；HTML 中的 WOFF2 不因封装而改用 AGPL。网站构建同时附上 `assets/OFL.txt`、各 `assets/*-OFL.txt`、`font-sources.json` 和本清单，保留样例原字节及其历史哈希 |
| 浏览器内含第三方代码及声明 | Chromium、ungoogled-chromium 及构建维护方；不同组件采用 BSD/MIT/Apache 等各自许可 | `renderer/browser-licenses/` 保留原文及 Windows/Linux credits；完整运行时中的 credits/legal 同时保留。清单不把整个浏览器统一改为 AGPL |

上述字体例外逐项覆盖 `assets/NotoSansSC.ttf`、`NotoSansTC.ttf`、`NotoSansJP.ttf`、`NotoSansKR.ttf`、`NotoEmoji.ttf`、`MDcardDiagramSans-Regular.ttf`、`MDcardDiagramSansKR-Regular.ttf`、`MDcardUISansJP-Regular.ttf`、`MDcardUISansTC-Regular.ttf`，以及由它们生成、复制或嵌入的字体文件。即使文件以 MDcard 命名，仍保留上游 Adobe / Google 等版权和 OFL。字体生成脚本本身属于项目自有代码。

`THIRD_PARTY_NOTICES.txt`、`renderer/THIRD_PARTY_NOTICES.txt`、`renderer/Node-LICENSE.txt`、`renderer/PlantUML-MIT-LICENSE.txt`、`renderer/browser-licenses/`、`assets/OFL.txt`、`assets/*-OFL.txt` 和 vendor 原许可证是第三方文本的保留位置，其中的 MIT 等标识不表示本项目仍提供 MIT 备选许可。锁文件中第三方条目的 `license` 字段同样保留上游事实。

网站和图标工具用到的外部开发工具不随静态页面或图标本体作为源码复制；安装这些工具时取得的依赖仍按上游许可使用。Python `requirements.txt`、npm lock 与 Rust lock 负责固定相应版本，不能据此声称这些工具由 MDcard 原创。

新开发者构建不需重新生成字体：原始字体和已改名的静态实例均已公开。重生成需安装 assets/branding/mdcard-icon/tools/requirements.txt 或上述 fonttools；输出须重新核对 name 表和 OFL。字体不是单独售卖的产品。

本次统一授权修订不改写第三方源码原许可、字体、运行时或已发布包的二进制字节。制作新应用包时，打包入口携带项目授权声明、本清单和 vendored egui_commonmark 的原文及修改说明；精简包将这些文本去重收入 `licenses/` 并在 `LICENSES.txt` 标明原路径。仍须按实际分发组件满足其对应源码、重链接等适用义务，不能只复制本表。
