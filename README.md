# MDcard 0.1.0

面向中文技术写作者的 Rust 原生 Markdown / 文本桌面编辑器，提供编辑、分屏、阅读、结构编辑、跨文件搜索、阅读位置恢复和本地公式图表渲染。

发行更新与校验进行中，应用下载暂停 · [产品页](https://mdcard-document-journey.wholeangel11.chatgpt.site/) · [构建说明](docs/DEVELOPMENT.md) · [文档导航](docs/README.md) · [报告问题](https://github.com/huwany1/MDcard/issues) · [贡献指南](CONTRIBUTING.md)

## 安装与运行

Windows / Linux 0.1.0 正在完成发行准备和公开下载闭环，完成前不提供应用压缩包下载。

两平台采用同一冻结源码构建，完整解压后即可运行，保留 `renderer`、许可证和 `manifest.json`。无需另装 Node、Java、Chromium、Pandoc 或开发工具。

Windows 11 x64：运行 `MDcard.exe`。默认 OpenGL 不可用时自动使用 Windows 内置 CPU 图形后端，保留原 GPU 偏好；程序版本可用 `MDcard.exe --version` 查询。Windows 10 未作本轮支持声明。

Linux x64：Debian 12 / glibc 2.36、X11 桌面，运行 `./mdcard`。系统需桌面图形/字体/音频库和可工作的原生文件对话框 portal；这些是系统桌面前提，应用运行时已随包提供。Debian 12 依赖：

```sh
sudo apt-get install libx11-6 libxcb1 libxkbcommon0 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libgl1 libegl1 libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libatspi2.0-0 libcups2 libdrm2 libdbus-1-3 libgbm1 libasound2 libpango-1.0-0 libcairo2 libfontconfig1 libfreetype6 xdg-desktop-portal xdg-desktop-portal-gtk
```

校验资产与下载入口将在完整上传和下载复验后恢复；请勿使用历史应用包的校验值验证新包。两平台真实构建源为 [c453689171b0](https://github.com/huwany1/MDcard/tree/c453689171b09463c57cc217fa48974a080854f2)，自有应用版本统一 0.1.0。逐文件白名单与用途、大小和 SHA256 位于包内 `manifest.json`；完整验收边界见[本次验收报告](https://github.com/huwany1/MDcard/blob/main/docs/releases/0.1.0-acceptance.json)和[正式发行索引](docs/releases/latest.json)。

本包包含路径别名重命名、ELK 隔离、xmldom 修复与最新已交付功能；本地公式图表和全部九种导出格式使用随包引擎。用户稿件、设置、源码快照、开发与测试工具、品牌交接资料和历史报告不随应用包分发。历史 MIT 授权继续保留；新应用包自有部分为 AGPL-3.0-only。

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

历史 Windows 验收使用 Windows 11 / 125% DPI，Linux 使用 Debian 12 / X11；其他 DPI、Wayland、桌面和驱动组合未全面验证。没有 macOS 正式包。历史性能数据仅适用于其记录环境，不代表所有设备或物理屏幕延迟。本次开源验证的命令和结果见[验收报告](docs/open-source/ACCEPTANCE.md)；后续方向见[成熟度路线图](docs/MATURITY_ROADMAP.md)。

源码导入基线与旧发行包二进制身份分别记录；本次构建不会声称与旧包二进制逐字节一致。原始本地日志、截图、部署配置与私人状态不公开，历史文档是设计和验收摘要，不能视作本次重新运行。

## 许可证

本次公开源码导入起，MDcard 自有代码、脚本、网站和自有图标采用 **AGPL-3.0-only**，完整正文见 [LICENSE](LICENSE)。第三方 Rust/JS 依赖、字体及运行时继续采用各自协议，见[授权边界与历史 MIT 说明](docs/LICENSING.md)、[第三方来源](docs/THIRD_PARTY.md)及各版权/NOTICE 文件。

分发受 AGPL 覆盖的程序或衍生作品时，应按协议保留声明并向接收者提供对应源码；修改版本通过网络与用户交互时，须按第 13 条向这些用户显著提供免费取得该版本对应源码的机会。单纯运行、未触发分发或该网络义务的私人修改、独立作品和用户稿件不因此自动变成 AGPL。不能推导为“采用者的所有代码都必须公开”。此前已获得的 MIT 授权不被撤销，已有发行包以随附许可证识别。具体以协议正文为准。
