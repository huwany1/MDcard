# MDcard 0.1.0

面向中文技术写作者的 Rust 原生 Markdown / 文本桌面编辑器，提供编辑、分屏、阅读、结构编辑、跨文件搜索、阅读位置恢复和本地公式图表渲染。

[下载正式包](https://github.com/huwany1/MDcard/releases/tag/v0.1.0) · [产品页](https://mdcard-document-journey.wholeangel11.chatgpt.site/) · [构建说明](docs/DEVELOPMENT.md) · [文档导航](docs/README.md) · [报告问题](https://github.com/huwany1/MDcard/issues) · [贡献指南](CONTRIBUTING.md)

源码开发入口为当前仓库根目录。构建产物、下载运行时、原始验收与部署记录不属于源码输入；品牌可编辑源、实际网站素材和必要许可保留。

## 安装与运行

- Windows x64：[MDcard-0.1.0-windows-x64.zip](https://github.com/huwany1/MDcard/releases/download/v0.1.0/MDcard-0.1.0-windows-x64.zip)，400,903,844 字节（382.33 MiB）。
- Linux x64：[MDcard-0.1.0-linux-x64.tar.gz](https://github.com/huwany1/MDcard/releases/download/v0.1.0/MDcard-0.1.0-linux-x64.tar.gz)，387,335,216 字节（369.39 MiB）。

两平台采用同一冻结源码构建，完整解压后即可运行，保留 `renderer`、许可证和 `manifest.json`。无需另装 Node、Java、Chromium、Pandoc 或开发工具。

Windows 11 x64：运行 `MDcard.exe`。默认 OpenGL 不可用时自动使用 Windows 内置 CPU 图形后端，保留原 GPU 偏好；程序版本可用 `MDcard.exe --version` 查询。Windows 10 未作本轮支持声明。

Linux x64：Debian 12 / glibc 2.36、X11 桌面，运行 `./mdcard`。系统需桌面图形/字体/音频库和可工作的原生文件对话框 portal；这些是系统桌面前提，应用运行时已随包提供。Debian 12 依赖：

```sh
sudo apt-get install libx11-6 libxcb1 libxkbcommon0 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libgl1 libegl1 libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libatspi2.0-0 libcups2 libdrm2 libdbus-1-3 libgbm1 libasound2 libpango-1.0-0 libcairo2 libfontconfig1 libfreetype6 xdg-desktop-portal xdg-desktop-portal-gtk
```

[SHA256SUMS.txt](https://github.com/huwany1/MDcard/releases/download/v0.1.0/SHA256SUMS.txt) · [发行元数据](https://github.com/huwany1/MDcard/releases/download/v0.1.0/MDcard-0.1.0-release.json)。两平台真实构建源为 [c453689171b0](https://github.com/huwany1/MDcard/tree/c453689171b09463c57cc217fa48974a080854f2)，自有应用版本统一 0.1.0。逐文件白名单与用途、大小和 SHA256 位于包内 `manifest.json`；发行身份以该 Release 的元数据和包内 manifest 为准，网站使用 `website/src/data/releases.json`。

本包包含路径别名重命名、ELK 隔离、xmldom 修复与最新已交付功能；本地公式图表和全部九种导出格式使用随包引擎。用户稿件、设置、源码快照、开发与测试工具、品牌交接资料和历史报告不随应用包分发。项目自有源码与资产统一采用 AGPL-3.0-only；第三方内容见[来源与例外清单](docs/THIRD_PARTY.md)。

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

历史 Windows 验收使用 Windows 11 / 125% DPI，Linux 使用 Debian 12 / X11；其他 DPI、Wayland、桌面和驱动组合未全面验证。没有 macOS 正式包。历史性能数据仅适用于其记录环境，不代表所有设备或物理屏幕延迟。当前开发步骤见[开发说明](docs/DEVELOPMENT.md)，模块约束见[维护地图](docs/ARCHITECTURE.md)。

源码导入基线与旧发行包二进制身份分别记录；本次构建不会声称与旧包二进制逐字节一致。原始本地日志、截图、部署配置与私人状态不公开，历史文档是设计和验收摘要，不能视作本次重新运行。

## 许可证

MDcard 当前版本的全部自有源码与资产，包括代码、脚本、配置、测试、文档、翻译、示例、网站、品牌设计、图标、图片和截图，统一采用 **AGPL-3.0-only**，不再提供 MIT 备选授权。全仓声明见 [COPYRIGHT](COPYRIGHT)，完整正文见 [LICENSE](LICENSE)，具体覆盖范围见[统一授权说明](docs/LICENSING.md)。第三方源码、依赖、字体及其派生、嵌入素材和运行时保留原许可，集中列于[第三方来源与例外清单](docs/THIRD_PARTY.md)。

分发受 AGPL 覆盖的程序或衍生作品时，应按协议保留声明并向接收者提供对应源码；修改版本通过网络与用户交互时，须按第 13 条向这些用户显著提供免费取得该版本对应源码的机会。用户稿件仍由其作者决定授权。历史版本的授权与本次修订范围见[统一授权说明](docs/LICENSING.md#历史版本)，具体权利义务以协议正文为准。
