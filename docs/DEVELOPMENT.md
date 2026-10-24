# 从全新克隆构建

本轮参考工具链为 Rust 1.92.0、Node.js 24、npm、Python 3.11+、Java 17+。Cargo.toml 的 rust-version 1.88 是声明下限，本轮未重验 1.88；不要把元数据下限视为整份锁文件已在该版本验证。网络安装会访问 crates.io、npm、Playwright、GitHub 等官方源；应用不会自动替你下载完整运行时。

## 桌面程序

先安装 Git 与 Rust。Windows x86_64 安装 Visual Studio Build Tools 的 Desktop development with C++ 和 Windows SDK；使用 Developer PowerShell（须能找到 cargo、link.exe、rc.exe）。rc.exe 用于编译仓库自有图标；无需维护者本机路径。GNU 交叉编译需 MinGW windres，不能替代真实 Windows GUI 验收。

Debian/Ubuntu 构建依赖：

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libegl1-mesa-dev libxrandr2 libxcursor1 libxi-dev libxkbcommon-x11-0 libgl1 libegl1 libvulkan1 zenity xdg-desktop-portal xdg-desktop-portal-gtk
git clone https://github.com/huwany1/MDcard.git
cd MDcard
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
cargo run --locked
```

Windows 执行相同 cargo 命令，产物为 target/release/mdcard.exe；Linux 为 target/release/mdcard。字体、图标、vendor、locales 已包含在源码中，编译不依赖未公开目录。仅构建桌面壳不安装 renderer 也可完成；公式和导出须继续下面步骤，缺少运行时会报错。

Dockerfile 是 Debian 12 / Rust 1.92 的可选开发环境，WORKDIR /work 是容器挂载约定：

```sh
docker build -t mdcard-build .
docker run --rm -v "${PWD}:/work" mdcard-build cargo test --locked
docker run --rm -v "${PWD}:/work" mdcard-build cargo build --locked --release
```

Docker 构建不是桌面显示测试。Wayland、其他发行版、macOS、其他 CPU/驱动组合未在本次验证。Linux 运行依赖还可能包括 libnss3、libgbm、libasound（供 Chromium 使用），安装浏览器依赖可使用下文 Playwright --with-deps。

## 本地渲染器

```sh
cd renderer
npm ci --ignore-scripts --no-audit --no-fund
npm run build
cd ..
python scripts/prepare-development.py --browser
```

npm 默认只安装当前主机 Sharp 的可选二进制，不再强制安装另一平台包。build.mjs 生成 renderer/vendor 的 JS/CSS、字体和 bundle 输入清单。prepare-development.py 从 runtime-sources.json 的官方地址取得 MIT PlantUML jar，验证固定 SHA256 后原子放到 vendor；--browser 下载锁定 Playwright Chromium 到 renderer/runtime/browsers。需要 Java 17+ 在 PATH，或显式设置 MDCARD_JAVA。系统 Node 在 PATH 即可，应用优先使用随包 Node，源码开发不必下载两平台 Node。

Linux 如缺浏览器系统库：

```sh
cd renderer
PLAYWRIGHT_BROWSERS_PATH=./runtime/browsers npx --no-install playwright install --with-deps chromium
cd ..
```

从源码运行应用时会设置其自己的 PLAYWRIGHT_BROWSERS_PATH 指向该目录。直接跑 renderer 测试也须设置：

```sh
cd renderer
PLAYWRIGHT_BROWSERS_PATH=./runtime/browsers npm test
node --test export-process.test.mjs
```

PowerShell 对应为：

```powershell
cd renderer
$env:PLAYWRIGHT_BROWSERS_PATH = "$PWD/runtime/browsers"
npm.cmd test
node --test export-process.test.mjs
```

MDCARD_CHROMIUM 可为公式/图表 worker 指向现有 Chromium/Chrome/Edge；MDCARD_JAVA 可指向 Java。MDCARD_RENDERER_DIR 可显式指定 renderer 位置。默认从源码目录运行即可，不应引用维护者的磁盘目录。新 bundle 不包含 EPL ELK 布局器，指定 ELK 会明确报错；默认 flowchart/Dagre、sequence、mindmap 不依赖此布局。测试检查实际构建输入无 elkjs，并真实渲染这些默认图表。

HTML/PDF/PNG/JPEG、DOCX/EPUB 与 Marp 各有额外管线。Word/EPUB 或显式 Pandoc 管线需要单独安装 Pandoc，可设置 MDCARD_PANDOC；Java 仅 PlantUML 需要。完整便携包的跨平台资源准备入口为 scripts/prepare-rich-runtime.py 与 scripts/prepare-export-runtime.py（会下载 Node/Java/Chromium/Pandoc，固定来源/哈希见 renderer 两份 runtime-sources JSON，量大且需要网络）。最小主机步骤可编辑文档并运行公式/图表；桌面导出当前仍要求 renderer/runtime/node（Windows 为 node.exe）及 runtime/<platform>/chromium 的完整资源，不能把 worker 的系统浏览器配置当成导出的配置。需要完整导出时，从仓库根运行 `python scripts/prepare-rich-runtime.py`（Java须在PATH）与 `python scripts/prepare-export-runtime.py --platform windows`，Linux 将参数改为 linux；不要省略后暗中依赖旧 dist。资源脚本下载会较大，原始许可和 runtime legal 目录须保留。新包必须重新验收，禁止用这些脚本覆盖既有 Releases。

## 网站

网站是独立静态 HTML/CSS/ES module 源码，构建不需要 Rust、不读取上级 dist、不需要账号或部署凭据：

```sh
cd website
node scripts/build.mjs
node scripts/serve.mjs
```

打开 http://127.0.0.1:4173/，Ctrl+C 停止。npm run build / npm run preview 是别名。产物为 website/dist，可发布到任何静态托管服务；不用原维护者 .openai 配置。构建输入 src/data/releases.json 已包含真实下载身份，普通构建不重新生成发行数据。

浏览器检查需 npm ci --ignore-scripts 和 Playwright Chromium，并另开预览服务器；可用 CHROMIUM_PATH 指定浏览器，BASE_URL 指定预览地址。详细步骤见[网站说明](../website/README.md)。已有截图和导出样本是公开演示稿，不是私人稿件。

## 检查与历史工具

主 CI 执行 Windows/Linux cargo 检查、renderer Linux 真实引擎/进程测试、网站构建；运行状态看 GitHub Actions，排队不算通过。tests 下独立 Cargo 工程含锁文件和相对 source include，按需 cargo run/test --manifest-path tests/对应目录/Cargo.toml --locked。

scripts 内大量 verify-*.ps1 / verify-*.py 及 package-*.py 是保留的历史验收和候选打包工具。它们可能写死旧候选路径、/work 或本地产品包，需按参数和环境调整，也可能需要 xdotool、Xvfb、pywinauto、Pillow、字体工具或独立证据目录。这些不属于干净编译的必要步骤；其历史通过不自动变成本次通过。当前开源验证见[报告](open-source/ACCEPTANCE.md)。

[已知依赖报告与本次修复](open-source/DEPENDENCY_RISKS.md)单独记录，不能把编译通过等同于无安全风险。

本次验证使用干净 Git 克隆；依赖注册表缓存和编译缓存可复用，但没有复制私人资源。可复现开发入口指依赖与输入可取得，不承诺所有机器二进制逐字节一致或旧包完全重现。
