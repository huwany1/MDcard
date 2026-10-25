# 项目统一授权：AGPL-3.0-only

自 2026-10-25 本次统一授权修订起，当前版本中本项目有权授权的全部源码和资产统一采用 **GNU Affero General Public License version 3 only**，SPDX 标识为 **AGPL-3.0-only**。本版本不再为项目自有内容提供 MIT 备选授权，也不包含“或更新版本”授权。全仓授权声明见根目录 [COPYRIGHT](../COPYRIGHT)，完整协议正文见 [LICENSE](../LICENSE)。版权为 2026 MDcard contributors。

## 覆盖范围

以下声明同时适用于未逐文件加入 SPDX 头的文本、数据和二进制文件；文件格式或存放目录不构成自有内容的许可例外。

| 本项目自有内容 | 对应路径或形式 | 许可 |
| --- | --- | --- |
| 桌面应用与本地渲染、导出代码 | `src/`、`build.rs`、`renderer/` 中的自有实现 | AGPL-3.0-only |
| 自有构建、打包、发布与验收工具 | `scripts/`、`.github/`、Dockerfile、自有工具脚本 | AGPL-3.0-only |
| 测试、合成夹具、配置与元数据 | `tests/`、Cargo/npm 自有包元数据、锁文件中的项目自有部分及项目配置 | AGPL-3.0-only |
| 文档、翻译、示例与公开演示稿 | 根目录文档、`docs/`、`locales/`、`examples/`、`website/samples/` 的自有部分 | AGPL-3.0-only |
| 品牌设计、图标、图片和截图 | `assets/branding/` 的自有设计及 SVG/PNG/ICO、示例图片、网站图片与产品截图中的自有部分 | AGPL-3.0-only |
| 网站及生成资产 | `website/` 的自有源码、样稿、HTML/PDF/ZIP 导出和其他生成文件中的自有部分 | AGPL-3.0-only |
| 对上游组件的自有新增修改 | 例如 `vendor/egui_commonmark/` 中由 MDcard 持有版权的新增和修改；上游原内容及其声明继续保留 | 自有修改为 AGPL-3.0-only；上游内容仍按原许可 |

第三方代码、字体、字体派生及子集、运行时和嵌入素材不因放入上述目录而被改为 AGPL。来源、具体路径、原许可和保留位置集中记录在[第三方来源与例外清单](THIRD_PARTY.md)，各组件原始版权、LICENSE、OFL 和 NOTICE 保留。许可证正文及引文也保留其自身复制条件。

## 分发与使用

- 分发受覆盖作品时，携带 `LICENSE`、`COPYRIGHT`、本说明和第三方清单，并保留实际使用组件的完整版权、许可及 NOTICE；清单不能代替这些原文。精简应用包可去重存放原文，但必须在 `LICENSES.txt` 中逐项映射组件和原文件名。
- 按 AGPL 第 4—6 条为受覆盖程序或衍生作品提供对应源码及适用的构建、安装信息。二进制包中的源码链接须指向该包实际构建版本，不能用持续变化的分支代替。字体、图标等资产的可编辑源与生成脚本随源码保留。
- 修改版本通过计算机网络与用户远程交互时，按第 13 条显著提供免费取得该版本对应源码的机会。
- 用户自己撰写、编辑、阅读或导出的稿件仍按作者选择的许可处理。项目自有公开样稿是本次授权的对象；普通用户稿件不因使用本软件而自动改为 AGPL。输出中嵌入的第三方字体或素材仍须遵守其许可。
- 单纯运行、不涉及分发及上述网络交互的私人修改，以及独立作品，按协议各自判断，不意味着使用者的全部代码都必须公开。本说明不增加协议正文以外的限制。

## 历史版本

本次修订移除了当前目录中的项目旧 MIT 许可副本；历史材料引用的 MIT 只描述当时版本，不是本版本的双许可证选项。此前已按 MIT 获授的具体版本仍保留其原有权利，不能通过修改当前仓库追溯撤销。

Git 历史中的旧许可可由固定的[历史提交快照](https://github.com/huwany1/MDcard/blob/da9d8b43bce2d89a6932d5b5e1b7e31c4b2f5bcb/docs/open-source/LEGACY_MIT_LICENSE.txt)核对。本次修订更新当前源码与后续打包入口；已经发布或归档的旧包、哈希和历史验收事实不被改写，其身份仍以原包随附文件为准。

## 协议原文

[GNU AGPLv3](https://www.gnu.org/licenses/agpl-3.0.html)、[SPDX AGPL-3.0-only 标准文本与标识](https://spdx.org/licenses/AGPL-3.0-only.html)、[OSI 收录全文](https://opensource.org/license/agpl-3-0)。本说明用于明确本项目授权范围，具体权利义务以完整协议正文为准。
