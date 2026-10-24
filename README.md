# MDcard

面向中文技术写作者的 Rust 原生 Markdown / 文本桌面编辑器，提供编辑、分屏、阅读、结构编辑、跨文件搜索、阅读位置恢复和本地公式图表渲染。

[下载正式包](https://github.com/huwany1/MDcard/releases/tag/v0.1.1-20261024) · [产品页](https://mdcard-document-journey.wholeangel11.chatgpt.site/) · [构建说明](docs/DEVELOPMENT.md) · [文档导航](docs/README.md) · [报告问题](https://github.com/huwany1/MDcard/issues) · [贡献指南](CONTRIBUTING.md)

## 安装与运行

Windows x86_64：[Icon Suite R3 ZIP](https://github.com/huwany1/MDcard/releases/download/v0.1.1-20261024/MDcard-0.1.1-icon-suite-20261024-r3-windows-x64.zip)。完整解压后运行 `MDcard.exe`，保留全部伴随资源。

Linux x86_64：[R29 tar.gz](https://github.com/huwany1/MDcard/releases/download/v0.1.1-20261024/MDcard-0.1.1-precision-b-20261024-r29-linux-x64.tar.gz)。基于 Debian 12 / glibc 2.36，解压后运行 `chmod +x mdcard && ./mdcard`。Linux 桌面依赖和平台限制见[开发说明](docs/DEVELOPMENT.md)。

请用现有 [SHA256SUMS.txt](https://github.com/huwany1/MDcard/releases/download/v0.1.1-20261024/SHA256SUMS.txt) 校验。两个正式包分别约 576 / 626 MiB，构建编号不同，随附 Node、Chromium、Java、Pandoc、字体和各自许可证。本次源码开源不替换或重新许可这些既有包，旧包的 MIT 权利及第三方授权保持有效。

## 已有能力

- Write / Split / Read 三视图、浅色/深色/跟随系统、六语言界面及原生文件打开保存。
- Markdown 与常见文本/代码文件；多编码选择和转换保护；单文件上限 16 MiB。
- 表格和块操作、源码同步、撤销/重做、目录树、Quick Switcher、工作区搜索替换、Diff 预览和替换历史回滚。
- KaTeX、Mermaid 默认 Dagre / sequence / mindmap、PlantUML MIT 构建和 flowchart.js；本机渲染；新源码构建明确不含 ELK 布局。
- HTML、PDF、图片、Word、EPUB 和 Marp 导出。各格式有独立边界，不能承诺逐像素一致。
- 每份已保存文件的阅读位置、最远位置与已读状态分别记录；内容变化后的定位存在预算和歧义降级。

普通预览不是任意富文本编辑器。支持的表格/公式/图表通过结构工具与源码联动；网页演示不具备桌面文件功能。细节见[模块文档](docs/README.md)。

## 从源码开始

需要 Git、Rust 1.92.0、Node.js 24、Python 3.11+；Windows 还需 Visual Studio C++ 与 Windows SDK（在开发者 PowerShell 运行），Linux 需本地桌面开发库。没有维护者私有目录或凭据要求。

```sh
git clone https://github.com/huwany1/MDcard.git
cd MDcard
cargo build --locked
cargo test --locked
```

纯桌面编译使用仓库内字体、图标和 Rust 锁文件。公式、图表、完整导出需要另外准备本地 renderer：

```sh
cd renderer
npm ci --ignore-scripts --no-audit --no-fund
npm run build
cd ..
python scripts/prepare-development.py --browser
cargo run --locked
```

PlantUML 需 Java 17+；Word/EPUB 等可选管线需 Pandoc。完整命令、额外下载、资源校验、Docker 和网站独立构建见[开发说明](docs/DEVELOPMENT.md)。

## 当前限制与证据边界

当前不含自动保存和稿件崩溃恢复；原子保存、外部修改检测和关闭提示不能替代备份。阅读位置状态不包含草稿正文。HTTP(S) 图片和点击外链可能联网；不能把本机渲染概括为应用永不访问网络。

[历史 Windows 验收使用 Windows 11 / 125% DPI，Linux 使用 Debian 12 / X11；其他 DPI、Wayland、桌面和驱动组合未全面验证。没有 macOS 正式包。历史性能数据仅适用于其记录环境，不代表所有设备或物理屏幕延迟。本次开源验证的命令和结果见验收报告](docs/open-source/ACCEPTANCE.md)；后续方向见[成熟度路线图](docs/MATURITY_ROADMAP.md)。

源码导入基线与旧发行包二进制身份分别记录；本次构建不会声称与旧包二进制逐字节一致。原始本地日志、截图、部署配置与私人状态不公开，历史文档是设计和验收摘要，不能视作本次重新运行。

## 许可证

本次公开源码导入起，MDcard 自有代码、脚本、网站和自有图标采用 **AGPL-3.0-only**，完整正文见 [LICENSE](LICENSE)。第三方 Rust/JS 依赖、字体及运行时继续采用各自协议，见[授权边界与历史 MIT 说明](docs/LICENSING.md)、[第三方来源](docs/THIRD_PARTY.md)及各版权/NOTICE 文件。

分发受 AGPL 覆盖的程序或衍生作品时，应按协议保留声明并向接收者提供对应源码；修改版本通过网络与用户交互时，须按第 13 条向这些用户显著提供免费取得该版本对应源码的机会。单纯运行、未触发分发或该网络义务的私人修改、独立作品和用户稿件不因此自动变成 AGPL。不能推导为“采用者的所有代码都必须公开”。此前已获得的 MIT 授权不被撤销，已有发行包以随附许可证识别。具体以协议正文为准。
