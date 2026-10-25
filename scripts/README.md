# 当前维护工具

从仓库根运行；脚本按自身位置定位源码，不要求维护者的磁盘路径。`python scripts/check.py` 是常规开发入口，可以组合 `--desktop --acceptance --renderer --website`；依赖准备见[开发说明](../docs/DEVELOPMENT.md)。输出位于被忽略的 `target/`、`artifacts/`、`evidence/` 或 renderer 的生成目录。

| 任务 | 入口与前提 |
| --- | --- |
| Rust 常规检查与独立回归 | `check.py --desktop --acceptance`；Rust 工具链、平台编译器 |
| 真实引擎、取消和 CLI 回归 | `check.py --renderer`；先 npm ci、准备 PlantUML/Chromium，Java 在 PATH |
| 独立 SVG/主题/IPC 检查 | `node scripts/verify-rich-independent.mjs`；与 renderer 相同依赖 |
| 完整导出回归 | `verify-export.mjs`、`verify-export-semantics.mjs`、`verify-export-svg-fonts.mjs`、`verify-export-theme.mjs`；先准备完整主机导出运行时，Word/EPUB 需 Pandoc |
| 运行时准备 | `prepare-development.py`（主机开发最小输入）；`prepare-rich-runtime.py`、`prepare-export-runtime.py --platform windows\|linux`（完整导出输入）；固定来源和 SHA 在 renderer JSON |
| 原生基本流程 | Windows `pwsh -STA -File scripts/verify-windows.ps1 -Exe <release-exe>`；Linux `sh scripts/verify-linux.sh`，需要 Xvfb、xdotool、ImageMagick 和图形库 |
| 结构 GUI 回归 | `verify-structure-windows.ps1 -Exe <release-exe>`、`python3 scripts/verify-structure-linux.py <release-exe>`；Windows 需要已解锁的 125% DPI 桌面，Linux 使用独占 Xvfb |
| GPU/CPU 回退 | `verify-ui-gpu-windows.ps1 -Exe <release-exe>`、`sh scripts/verify-ui-gpu-linux.sh`；Linux 需要 Mesa Vulkan CPU 驱动与 zenity；脚本独占各自测试显示 |
| 原生性能测量 | `benchmark-large-document.py`、`benchmark-large-code.py`、`benchmark-large-media.py`、`verify-large-document-master.py`；Linux Xvfb/Openbox/xdotool，先构建 target/release/mdcard；media 还需 renderer |
| 独立阅读匹配语料 | `generate-reading-precision-master-fixtures.py --output <dir>`，运行 `tests/reading-precision-acceptance` 的 adapter，再用 `score-reading-precision-master.py --help` 对冻结答案评分 |
| 字体和编码夹具 | `generate-diagram-font.py`、`generate-ui-fonts.py`（fonttools 4.61.1）、`generate-encoding-fixtures.py`；常规编译不必重生成，改动后复核来源及哈希 |
| 声明与公开范围检查 | `collect-licenses.py`、`renderer/licenses.mjs`；`audit-publication.py --index-file <candidate-index> --output <local-json>` 扫描隔离候选索引（省略 index-file 则扫描当前索引），`--ancestor <ref>` 可另扫历史；模式匹配不保证发现所有隐私信息 |
| Windows 快捷方式 | `Create-MDcardShortcut.ps1 -AppDirectory <complete-app-directory>`；只在明确需要创建桌面入口时执行 |

GUI 脚本会控制鼠标/键盘，仅运行隔离的测试实例；不要与真实编辑同时操作桌面。测试环境、DPI、软件渲染及物理呈现延迟各自记录，历史结果不能替代新运行。Linux 工具可能需在容器中使用 `-v "${PWD}:/work" -w /work`，挂载位置可变，不要求名为 mdcard-dev 的既有容器。

旧候选包、发布替换、清理迁移和阶段证据脚本不再作为当前入口。其原件保留在本地仓外归档，旧公开版本仍可从 Git 历史取得；本次不修改既有 Release。后续发布工具应针对新目录构建并重新验收，不复用旧 dist 作为隐含输入。
