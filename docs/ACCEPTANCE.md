# MDcard 本地验收与历史证据索引

当前 Windows 正式版本为 Icon Suite R3，Linux 正式版本为 R29，入口为 `dist/latest.json` 与 `dist/windows`、`dist/linux`。Windows 图标、完整包哈希及真实桌面验证见[图标验收](ICON_INTEGRATION_ACCEPTANCE.md)；R29 阅读精度的原38项与支持范围见[精度验收](READING_PRECISION_ACCEPTANCE.md)和[历史交付](READING_PRECISION_DELIVERY.md)。文档导航及清理后路径去向见[文档入口](README.md)。本文后续各历史模块记录保留采集时的版本身份，不作为新版本重复验收声明。

## 上一快照（2026-10-22）

旧工作台及阅读状态首阶段的运行包已由R29替代，原始验收记录仍保留。旧路径清理和当前全部留存理由见 [本轮清理](RESOURCE_CLEANUP_2026-10-24.md)；此前清理快照见 [历史清理](RESOURCE_CLEANUP_2026-10-22.md)。

下方各模块的 0.1.0 测试数、哈希、路径和平台结论均为各自历史版本记录。旧验收目录和旧模块包在清理后可能已不存在，不能把这些历史记录当成本轮重新执行或当前候选包通过的证明。

## 历史增量：全局搜索、Quick Switcher 与正则替换（2026-10-20）

该历史交付基于 `codex/global-search`，采用一个实现子代理与主代理独立验收。GS01–GS05 的契约、操作入口、后台管线、预算/冲突/事务边界见 [全局搜索说明](GLOBAL_SEARCH.md)。当时交付为独立 `dist/MDcard-0.1.0-search-windows-x64.zip`、`dist/MDcard-0.1.0-search-linux-x64.tar.gz`，完整包含既有本地图表 renderer 与许可证；当时 rich 包保持其原版本。这些旧模块包现已清理，当前正式归档见上节。

| 验收门槛 | 证据 |
|---|---|
| 1600+ 文档工作区 <50ms 检索 | `evidence/search/master-review/performance*.json`；真实 GUI 输入 changed→绘制结果（含 worker/UI poll）100 轮、1600 文件与增删改，`evidence/search/linux-ui-performance/performance.json` |
| Quick 模糊/权重/MRU、上下键/Enter/Shift+Enter、缺失新建 | Windows 与 Linux 实际鼠标键盘及截图：`evidence/search/master-review/windows`、`evidence/search/linux-final` |
| 捕获组、多行、Aa/全词、Glob、预览后单条/文件/全部替换 | 核心独立用例和 GUI diff；磁盘/两个内存文档同步，单文档一次 Ctrl+Z / Ctrl+Shift+Z |
| 无效正则、ReDoS、取消/截止 | 内联错误截图；八类 16MiB 对抗输入与展开 NFA 工作预算测试；不会把不完整/被截断的预览提交 |
| 冲突/失败/回滚 | dirty + pending 编码/BOM 恢复、stale diff/外改拒绝、真实第 2 文件注入写失败、补偿失败后的 `recovery_required`、跨重启恢复；`tests/search-acceptance`、`src/search.rs` 单测与 GUI restart rollback |
| 六语言、编译/格式/静态检查 | `evidence/search/linux-final` 六语言搜索/Quick 截图；`evidence/search/core-linux.txt`、`clippy-linux.txt`、`fmt-linux.txt`；Windows 独立核心日志 `evidence/search/master-review/core-windows.txt` |

平台实证为 Windows 本机与 Debian/X11/Openbox/Mesa，未扩大到未测试的 Wayland/macOS/输入法或所有硬件。检索 SLA 为报告内语料和查询分布，初次索引时显示进度；任意复杂正则受明确工作/输出预算限制。跨文件提交使用 prepared/committed/recovery_required 历史与逐文件原子写，没有声称多文件文件系统原子性或断电后目录项持久性。最终哈希、run UUID、性能分位数与清理状态以对应 JSON 为准。

## 既有增量：数理公式与代码化图表（2026-10-20）

原生公式同行渲染、节点源码联动、真实多引擎图表、双向无损补丁、主题/安全/缓存与视口懒加载。冻结架构、最终程序hash、性能边界与重现步骤见 [模块验收说明](MATH_DIAGRAMS.md)。本轮证据为 `evidence/rich/windows-final`、`evidence/rich/linux-release`；独立验收为 `evidence/rich/master-review`，发行清单为 `evidence/rich/packages`。是否通过以对应新鲜报告为准，不替代下方历史版本结论。

验收日期：2026-10-19（Asia/Shanghai）。实现与验收由 1 个实施子代理、1 个主代理完成；主代理独立复查界面、文件内容与构建哈希。下列结论限于实际运行环境。

## 可视化表格与块操作（历史增量）

当前范围与验收矩阵见 [结构操作说明](STRUCTURAL_EDITING.md)。原生表格网格、行列右键与按钮、快捷键、块六点手柄拖拽、列表层级及共用原子撤销均已实现。当前源码 Linux54/54、Windows原生53/53 单元测试通过，0 failed/ignored，fmt/clippy通过。新证据仅使用 `evidence/structure/windows-final/`、`linux-final/`、`master-review/` 与专属 manifest；早期开发截图不作为当前源码证明。Windows125% DPI 与 Linux X11 原生 GUI 覆盖源码/结构同步、磁盘内容、空边缘表格保形、真实拖拽、列表快捷键与撤销/焦点；Linux另覆盖六语言最小窗及1MiB顺序100命令、20ms间隔100新增行burst。压力burst无丢命令但CPU更新P95约115ms，详细边界见结构说明。以下编码等章节均属各自旧哈希历史范围。

## 多主流文本编码（历史增量）

2026-10-19 在当前未提交功能基础上增量实现；保留文件树、六语言、GPU、原生圆角及其既有变更。新证据仅使用 `evidence/encoding/windows-final/`、`linux-final/` 和 `evidence/encoding/manifest.json`；`windows/` 是开发试验截图，不作为最终验收依据。原有验收章节均为对应旧哈希的历史范围。

支持 UTF-8 无/有 BOM、UTF-16 LE/BE（保存时 BOM 可选）、GBK、GB18030、Big5、Shift_JIS、EUC-KR、Windows-1251、Windows-1252。自动规则为 BOM → 严格 UTF-8 → chardetng 推测；GBK 推测提升为 GB18030，以支持四字节扩展字符。旧编码带“推测”状态，普通保存不消除不确定性；用户明确选择后才取消推测标记。无 BOM UTF-16 需手动选择。UTF-32 BOM、冲突 BOM、奇数字节 UTF-16、非法代理对、解码后控制字符明确拒绝。

| 验收项 | 实际验证与证据 |
| --- | --- |
| 独立编码字节基准 | Python 标准 codecs 生成13组原文/编辑后 golden bytes，覆盖11选项及UTF-16无BOM，`tests/fixtures/encodings/`；Rust严格比较读取文本、未编辑原始字节、编辑后磁盘字节，不只用自身编码器互证 |
| 保存与无损边界 | 单元用例覆盖全部格式、混合换行、emoji/扩展汉字、BOM、GB18030自动识别后编辑、Big5重复映射原字节、Shift_JIS yen别名拒绝、不可表示字符、编码dirty、保存失败/外部冲突；保存成功前不清状态 |
| Windows真实交互 | `windows-final/result.json` 记录唯一UUID、实际exe哈希与隔离设置；GB18030推测 → 选择UTF-8 → dirty → 新建保护/取消 → 保存，与Python UTF-8字节完全一致；UTF-16无BOM自动失败 → 错误入口手选 → 打开 → 编辑保存，与Python UTF-16LE字节完全一致 |
| 重开保护 | `dirty-reopen-guard.png`、`dirty-cancel-retained.png` 为最终二进制的真实状态栏点击、重新打开、取消后保留dirty/文本截图；转换后的保护见`conversion-unsaved-guard.png`。重开失败只在读取成功后替换doc，保存/选择编码失败均保留编辑；Linux`unmappable-keeps-edits.png`实测不可表示字符错误 |
| 六语言与窄窗 | `locale-zh-CN.png` 至 `locale-ru.png`；`compact-*.png`为Windows125% DPI、680×500逻辑像素，含较长俄文与换行页脚；说明区滚动、底部按钮固定可见。翻译完整性、占位符、离线字形单测通过 |
| Linux真实运行 | Debian12/Xvfb X11/llvmpipe OpenGL，当前新二进制执行GB18030打开和UTF-8转换精确字节比较，退出0；`linux-final/result.json`、`encoding-grid.png`、`converted-utf8.png`；未声称Wayland或其他桌面已测 |
| 编译与检查 | Linux30 passed、Windows原生29 passed，0 failed/ignored；Linux多1项Unix链接用例。`fmt --check`、`clippy --all-targets -D warnings`通过，双平台release --locked构建；日志在本轮证据目录 |
| 主代理独立复核 | 重新运行Windows测试exe，29 passed /0 failed /0 ignored，1.45s；日志`artifacts/encoding-master-final-tests.log`；测试exe SHA256 `73934D7B1AF4F27042CDED23A0FCC68DD5C6A82F2F33859CDB6EB7112C675BD8` |

手动重开与保存转换分离；仅当前文档提供保存转换，另一个待打开文件只提供读取编码。原始BOM作为文件元数据；未编辑保存直接复用原始字节，规避旧编码非单射映射。编辑后要求严格编码、且回解文本完全一致，拒绝静默替代/转义。编码或BOM改变计dirty，原子写与外部修改检测继续生效。检测不确定性、WHATWG映射/厂商变体、UTF-32等不支持范围见README；不能把识别结果当成确定的源格式。没有扩大到自动恢复、自动保存或编码标准认证。补充平面汉字与emoji的字节已验证；当前内嵌字体不涵盖所有扩展汉字字形，截图中的罕见扩展字可能显示缺字框，不影响字节保持。

复验命令：`pwsh -NoProfile -File scripts/verify-encoding-windows.ps1`、`docker exec mdcard-dev sh scripts/verify-encoding-linux.sh`。Windows测试exe保留在忽略目录`artifacts/encoding/windows/encoding-tests.exe`，可直接运行；测试自带独立Python基准字节，不需要运行时源码路径。脚本使用隔离设置、专属进程/显示与临时夹具；最终容器/窗口清理状态见manifest。未覆盖旧历史截图与manifest。

## 自动后台索引与文件夹树（历史增量）

每次启动直接索引当前文档所在目录；无文件时索引启动工作目录。侧栏自动显示根、可展开/折叠的子文件夹和文本文件，名称在图标后左对齐，当前文件高亮，整行可点击；不需要先打开检索窗。点击子目录文件后，根立即切到新文件所在目录，未保存编辑仍经过原保护流程。只显示支持文件及其祖先目录，不显示空目录或仅含不支持类型的目录。

| 验收项 | 实际证据与边界 |
| --- | --- |
| 自动树、层次、展开折叠 | `evidence/folder-tree/linux/automatic-tree.png`、`expanded-folder.png`、`collapsed-folder.png`、`collapsed-root.png`；无 Ctrl+P 启动即显示。文件夹/文件图标及箭头直接绘制，避免缺字 |
| 点击、保护、保存、根迁移 | Linux 真实点击 `peer.txt`，编辑后点击另一文件触发保护，Escape 取消后保存精确字节比对；打开 `Alpha/child.txt` 后根为 Alpha，`dirty-cancel-dialog.png`、`cancel-preserved-and-saved.png`、`opened-child-root-rebased.png` |
| 自动与手动刷新 | 相同目录打开自动刷新；快速请求保留待刷新标记，最多两秒合并，当前扫描结束后补一次；不静默丢弃。刷新时保留原树与展开状态。`same-folder-automatic-refresh.png`、`explicit-refresh.png`，调度/缓存单测 |
| 无文件与窄窗 | `startup-working-directory.png` 验证无文档启动自动使用 cwd；小于850逻辑像素隐藏侧栏，Ctrl+P 可达；`narrow-window-search.png`。树不提供上级导航，遵循当前文件目录向下的原有边界 |
| 有限扫描、错误与截断 | 保留500,000条目/100,000文件/遍历15秒预算、跳过依赖/构建目录与链接，错误和部分索引直接在侧栏显示，悬停查看详情。排序和构树也在后台，不读取文件内容 |
| 10万文件缓存验证 | release单测创建100,000条路径，构建100,201节点，初始可见101行；Linux约96ms、Windows约92ms（以最终日志为准）。10,000次不变帧的缓存同步约20/11微秒；这是内存路径微基准，不是磁盘/网络速度承诺 |
| 检查 | Linux23/23、真实Windows22/22单测通过，0 failed/ignored；Linux多1项链接测试；fmt --check、clippy --all-targets -D warnings、双平台release --locked通过。日志在 `evidence/folder-tree/` |
| 独立Windows验收 | 主代理结果与最终二进制身份以 `evidence/folder-tree/master-review/result.json` 为准；当前包哈希绑定在 `evidence/folder-tree/manifest.json` |

目录树只在后台索引版本变化时接收结构，可见行只在版本或展开状态变化时重建；虚拟滚动只绘制视口，不在每帧遍历整个目录树。已有文件的普通内容保存不扫描目录，另存为只提出一次索引请求。没有添加持续文件监控。后台扫描无法中断文件系统单次阻塞 I/O；不将其15秒预算声称为强制停止时限。

Linux在Debian12/Xvfb/llvmpipe OpenGL真实运行并正常退出。软件渲染容器的总体CPU不能作为索引效率结论；历史软件渲染基线同样占用较多CPU，因此本轮性能结论限定为索引预算、后台工作、缓存及虚拟绘制。未重跑此前全部六语言/平台/窗口圆角矩阵，既有圆角源代码及打包流程保留，历史证据继续按原哈希区分。

复验：在项目根目录运行 `docker run --rm -v "${PWD}:/work" mdcard-build sh scripts/verify-folder-tree-linux.sh`，不依赖开发容器运行状态。测试使用独立设置、专属Xvfb和临时夹具并清理；保留截图、精确保存文件、日志与唯一UUID。Windows用户此前正在使用的旧实例保持运行，旧可执行文件仅可逆改名为 `dist/windows/MDcard-before-folder-tree.exe`，新版本发布在标准 `dist/windows/MDcard.exe`；备份旧exe不进入分发包。

## 顶层窗口原生圆角（历史增量）

本轮只增加 `src/window_corners.rs` 和顶层窗口生命周期调用，保留六语言、字体、文本检索/编辑及其未提交修改。最新证据独立位于 `evidence/window-corners/`；下方文本检索、本地化及 UI/GPU 记录各自保留历史二进制身份，不将其截图冒充本轮结果。

Windows 11 通过 DWM `DWMWA_WINDOW_CORNER_PREFERENCE` 设置普通窗口 `DWMWCP_ROUND`、最大化 `DWMWCP_DONOTROUND`，恢复后重新请求圆角。调用使用 eframe 提供的真实 HWND，仅在 HWND/最大化状态改变时执行；没有启用整窗透明、alpha 混合或 window region，不改变现有 Glow 与 WARP 渲染路径。实现依据 [Microsoft 原生圆角说明](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners) 和 [DWM 枚举定义](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_window_corner_preference)。系统可在贴靠/虚拟桌面/旧系统等情形下限制效果，API 是系统偏好请求，不承诺所有 OS 场景的圆角。

| 本轮检查 | 结果与证据 |
| --- | --- |
| 四角为真实窗口轮廓 | Windows 11 26100、125% DPI，Light/OpenGL 和 Dark/WARP 在纯粉色背景上进行实际桌面截屏，四角露出背景，窗口边缘及内部保持不透明；`windows/*-restored.png`、`*-restored-again.png`，不是内层面板或原生 surface 截图 |
| 最大化与恢复 | 两种后端真实绿色交通灯点击；DWM 属性由 2→1→2，Win32 `IsZoomed` 与最大化/恢复截图同步核对；`windows/result.json`、`*-maximized.png` |
| 现有窗口行为 | 最小化/恢复、标题拖动、右下角 resize、红灯 dirty 保护/Discard 关闭均在新二进制上通过；`windows/controls/result.json` 和截图。旧证据目录未覆盖 |
| Linux | 新二进制 CPU/Vulkan 实际启动、正常退出0；Xvfb/Openbox 无合成器，保持不透明矩形；`linux/opaque-x11-no-compositor.png`、`linux/result.json`。没有引入透明窗口或声称该环境圆角通过 |
| 编译与保留功能回归 | 双平台 release `--locked`；Linux 20/20、Windows 原生 19/19 单测通过，0 failed/ignored，包含六语言字形、保存保护与文本索引用例；fmt 与 clippy `-D warnings` 通过，日志均在本轮目录 |
| 主代理独立验收 | 额外 Dark/OpenGL、Light/WARP 两组合，在纯青色背景下真实屏幕截屏，逐角检查透出背景色相（含 DWM 阴影）、顶边保持不透明，读取 DWM Round=2、实际 renderer 并正常退出0；`master-review/result.json`、`dark-opengl.png`、`light-cpu.png`。与实施侧合计覆盖浅/深 × 两后端四组合 |

Windows SHA256 `C6F75DE227DEF10103B688AFE11E4211686C51723A31F0221A38FA805FA83C73`；Linux SHA256 `05B8E78AAC68EF1E18AB91E3E39253A6A637022B0B4665A298947142E7B90B30`。本轮可重放脚本为 `scripts/verify-window-corners-windows.ps1`（传入最终 exe 路径）与 `scripts/verify-window-corners-linux.sh`。最终包内身份由本轮 package-check 和 manifest 约束。

## 相对路径文本检索与编辑（前轮基线）

本轮在已有未提交本地化/外观实现之上增量修改，保留原保存保护。当前代码和二进制的证据位于 `evidence/text-files/`；下文的 `localization`、`ui-gpu` 和早期文件流程证据均为各自历史版本，不再作为本轮二进制哈希的验证。

| 验收项 | 实际结果与边界 |
| --- | --- |
| 文本类型与保存 | 统一 `text_files::supported` 用于打开与保存；原生选择器扩展筛选来自同一表，特定名称文件使用额外筛选；命令行、拖放和检索结果调用同一文档加载器。TXT/JSON/CSV/Rust/README/.env 的 UTF-8 BOM、中文/emoji、CRLF 修改保存单测通过 |
| Markdown 与原文展示 | MD 类保留 Markdown 预览/大纲；TXT/代码/配置等 Read 和 Split 预览按等宽原文展示，保留 #、**、HTML 标签；Linux `plain-read.png`，Windows 独立复核见 `master-review` |
| 相对路径与基准 | 当前文件所在目录为根，文件选择或另存为时迁移；无文件路径时为启动工作目录。显示真实根地址，大小写无关的文件名/相对路径子串过滤，支持中文、空格、两种斜线；拒绝父级与绝对路径。Linux 从 /tmp 启动但打开其他目录文件后正确检索；`linux/relative-results.png`、`current-folder-no-parent.png` |
| 后台检索和规模 | 仅后台遍历和路径索引，筛选无磁盘读，虚拟滚动结果；100,000 路径×3 查询在 debug 单测实测 Linux 80ms、Windows 84ms，仅为本机量级证据，非所有文件系统的时延保证 |
| 预算及失败 | 500,000 条目 / 100,000 文件 / 15 秒，达到时明确显示索引不完整；权限/目录错误明确计数并提供详情，空匹配有提示。预算、缺失根错误、外部新增后刷新、根切换取消旧索引有单测；没有声称真实多用户权限场景通过 |
| 目录边界 | 跳过文档列出的构建/依赖目录；不跟随符号链接和 Windows 重解析点。Linux 单测实际创建外部链接、目录循环、文件链接，确认均跳过；Windows 代码检查 reparse 标志，实际 junction 验证以主代理记录为准 |
| 内容边界 | 严格 UTF-8，拒绝常见二进制控制字节和无效编码；打开上限16 MiB。超限、伪装文本、非法 UTF-8 的失败不会覆盖当前文档。非 UTF-8 编码不在本轮范围 |
| 保存与交互保护 | 复用原子写入、外部修改检测及未保存确认；主代理真实切换检索结果时 Cancel 保留编辑并随后保存精确字节；刷新非空列表不崩溃。代码修复过刷新后旧下标越界、Ctrl+P 吞 Ctrl+Shift+P 两个回归，并保留回归单测与真实 UI 证据 |
| 构建与检查 | Rust 1.92；Linux release、Windows GNU release 均 `--locked` 构建通过；Linux 20/20、真实 Windows 原生 19/19 单测（Linux 多1个符号链接测试），0 failed/ignored；fmt --check、clippy --all-targets -D warnings 通过。日志均在 `evidence/text-files/` |
| 最终二进制身份 | Linux SHA256 `89FAD362A9798B30512FDC7D1D4C265F7EB8D70A003DCAF03701F79D89DCAD3F`；Windows SHA256 `EBCA57B36EC35CD7C88F4A337C066E34338D9006E7BAA6EED0F735CEAD3EF344`。最终复验和包哈希由本轮 `manifest.json` 及主代理最终确认记录约束 |

Windows 最终独立验收结果为 `evidence/text-files/master-review/result.json`，状态 passed，测试前后哈希一致；16–20 截图覆盖偏好快捷键、相对路径、原文、根迁移、刷新及紧凑窗口。未保存切换 Cancel 的真实证明保留于 `result-intermediate.json`，其运行时代码相同、二进制哈希不同，未冒充最终产物复验。Linux 最终 `linux/result.json` 状态正常退出0，真实修改 TXT 后与预期全字节比对相同，刷新后截图已目视确认。

可重放：`docker run --rm -v "${PWD}:/work" mdcard-build sh scripts/verify-text-files-linux.sh`；单测/构建使用 README 中的 Docker 工具链命令。Windows 独立键鼠验收截图和结果保留于 `evidence/text-files/master-review/`，其中中间版本与最终确认按 SHA256 区分，不复用旧二进制截图声称最终运行。

## 六语言本地化增量验收

当前交付语言为简体中文、繁體中文、English、한국어、日本語、Русский。应用界面共 82 条共享消息，每种语言均有完整目录；包括外观设置、语言设置、工具栏、视图、侧栏、提示、状态与动态计数、保存保护、文件错误和启动失败说明。欢迎文档为六份独立 CommonMark/GFM 示例。系统原生选择器按钮及 OS/驱动/底层库诊断按系统自身语言显示；应用提供的选择器标题和筛选名称随界面语言。保存不改变用户内容，也没有新增导出格式。

| 验收项 | 当前证据与边界 |
| --- | --- |
| 六语言完整目录与变量 | Rust 单测逐语核对消息键、非空值和 `{name}`/计数/路径占位符；每份欢迎文档包含真实表格、任务、链接和代码块 |
| 字形与混合语言 | 在不加载系统字体的 egui Context 中，逐一核对所有目录、欢迎文档和语言自称字符；比例字体 15px、等宽字体 23px 均无缺字。SC/TC/JP/KR 使用当前语言的区域字体优先 |
| Windows 六语言 UI | 最终二进制：`evidence/localization/windows/matrix.json` 及六语 workspace/preferences/compact/compact-preferences 共 24 张截图；Windows 125% DPI，正常 1240×820 与窄窗 620×420 逻辑尺寸，23px，Light / OpenGL |
| Linux 六语言 UI | `evidence/localization/linux/matrix.json`、`executable.sha256` 及同样 24 张截图；Debian 12 / X11 Xvfb / Openbox，正常与窄窗，23px，Dark / CPU Vulkan |
| 自动识别 | Windows 使用 GetUserDefaultLocaleName；Linux 按 LC_ALL、LC_MESSAGES、LANG 优先读取。Linux 实际 LC_ALL=zh_TW.UTF-8 且无设置文件时回报 zh-TW：`automatic-renderer.json`。单测包含大陆/新加坡/台湾/港澳/繁体脚本及 en/ko/ja/ru 与未支持语言 |
| 偏好兼容与原子保存 | 旧文件缺少 language 时跟随系统；未知语言仅回退自动，不丢失外观/GPU等设置；六种手动选择序列化往返；保存失败保持上次有效值。`evidence/localization/unit-tests.txt` |
| 文档保护 | 单测验证切换语言仅替换未修改的内置欢迎文档；不替换脏文档、已保存文件或新建空文档；六语言混合文本/Unicode 文件名/BOM/CRLF 保存重开字节不变 |
| 启动失败的本地化 | Linux 隔离 Vulkan 驱动导致实际启动失败，俄语原生错误窗及恢复说明可见，退出码 1，GPU off/俄语选择保留；`ru-startup-failure.png`、`failure-preserved-settings.json` |
| 构建与检查 | Rust 13/13 单测通过，0 failed/ignored；clippy --all-targets -D warnings 通过；Linux 和 Windows GNU release --locked 构建通过，日志见 `evidence/localization/` |
| 独立复核 | 主代理的语言选择、重启、文档保护、撤销与实际保存证据位于 `evidence/localization/master-review/`；以该目录 result.json 的记录为准 |
| 切换语言后撤销/重做 | `master-review/verify-undo.ps1`、`undo-result.json`：真实 en→ru 选择后 Ctrl+Z 恢复精确原文、Ctrl+Shift+Z 恢复精确编辑，分别保存后做字节比对。未切换语言对照在 investigation；egui 将光标也纳入撤销状态，复验先恢复粘贴时的末端光标，避免将一次光标回退误判为历史丢失 |

字体来自 Google Fonts 官方仓库，下载地址、大小、SHA256 保留于 `assets/font-sources.json`；三个新增字体的 OFL、现有 SC/Emoji OFL 均随 Windows/Linux 包附带。字体二进制直接内嵌，不需要联网或系统安装字体。分发包也附带六语言目录和欢迎文档，便于审阅。

Linux 原生选择器/启动错误窗依赖 UTF-8 locale；验收脚本使用 `LANG=C.UTF-8`。Docker 默认 ASCII locale 下，zenity 会拒绝含俄语的参数，因此该环境不计为原生对话框通过。应用内 egui 字形测试不依赖系统字体或系统语言。

以下外观/GPU/文件流程证据保留为前一轮历史记录，截图对应当时的旧二进制；本轮最终语言二进制身份由 `evidence/localization` 和重新生成的 manifest 约束，不将历史截图冒充最终版本验证。没有声称已完成母语使用者逐条审校、全部 OS/输入法组合或 Windows MSVC/真实 Linux 桌面会话验收。

## Mac 外观与 GPU 开关增量验收（历史）

本轮使用同一套原生 UI 完成 Light / Dark / System、六种强调色、统一 30px 控件基线、分段视图、主题化菜单/弹窗/链接/选区/代码、Mac switch 开关与交通灯符号。动画覆盖按钮悬停、交通灯、分段滑块、侧栏、视图、外观、弹窗与开关；Reduce motion 将过渡时间归零。原有文档保存与窗口行为继续使用同一实现。

| 验收项 | 实际结果与证据 |
| --- | --- |
| 浅/深色 × 六强调色 | Windows 真实点击 12/12，逐项核对持久化 JSON；`evidence/ui-gpu/windows/result.json` 与 `workspace-{light,dark}-{0..5}.png`、`preferences-{light,dark}-{0..5}.png` |
| Mac 对齐与紧凑窗口 | Windows 125% DPI，620×420 逻辑尺寸，工具栏/Read/菜单均可用；设置独立滚动、Done 固定可达；`compact-*.png` |
| 最大字号与滚动 | 23px 编辑/预览、窄窗排版、设置滚动至 GPU 一并实测；`font-size-23-dark.png`、`compact-font-size-23-dark.png`、`compact-preferences-gpu-scrolled.png`，`extras-result.json` |
| 深色脏文档与错误弹窗 | 粘贴后新建触发 Save/Discard/Cancel；启动打开非法 UTF-8 文件触发错误弹窗；`dirty-dialog-dark.png`、`error-dialog-dark.png` |
| 关闭 GPU → 重新启动 | Windows UI 真实关闭后保存、正常退出、无 CLI 覆盖再次打开；实际 `Microsoft Basic Render Driver · Cpu / Dx12`；`cpu-renderer.json`、`cpu-dark.png` |
| 开启 GPU → 重新启动 | Windows UI 真实开启并显示 Restart required；再次打开恢复 NVIDIA RTX 4060 Laptop / OpenGL 3.3；`gpu-on-pending.png`、`gpu-restored-renderer.json` |
| Linux CPU 模式与暗色内容 | Debian 12 / X11，实际 `llvmpipe · Cpu / Vulkan`，23px、中英文、图片/表格/任务列表/代码；`evidence/ui-gpu/linux/cpu-renderer.json`、`cpu-dark-23.png`、`dark-code-preview.png` |
| CPU 初始化失败 | 隔离 Vulkan driver 路径令软件后端不可用，系统独立错误窗可见、进程退出码 1、保存的 off 不变；`no-cpu-startup-dialog.png`、`no-cpu-startup.log`、`off-after-failed-start.json` |
| 动画 | Linux 侧栏前/中/后帧显示宽度变化；`sidebar-frame-{0..3}.png`、`sidebar-animation.gif`。Windows 真实切换 Reduce motion 并核对保存值；`reduce-motion-on.png` |
| 设置持久化与写失败 | 主题、强调色、字号、减少动画与 GPU 原子写入；失败恢复已保存值并提示。新增真实临时目录 roundtrip / 无法写入时旧文件完整保留的单测 |
| 构建与静态检查 | 双平台 release、Linux 和 Windows 7/7 单测、fmt 与 clippy `-D warnings`；`evidence/ui-gpu/` 日志 |

GPU-off 固定使用 wgpu CPU 适配器并验证 surface compatibility，不允许硬件 fallback。Windows 使用自带 WARP/Fxc，无需额外分发图形 DLL。Linux 使用系统 `libvulkan1` 与 `mesa-vulkan-drivers`；`zenity` 提供初始化失败时独立于渲染器的错误窗。加速开启请求 OpenGL，本机 Windows 确认为 NVIDIA 硬件；Linux 容器没有 GPU，OpenGL 仍为 Mesa 软件实现，不把它称作 Linux 硬件加速证明。

Ctrl+Shift+P 在 Windows 真实键盘验证通过，Ctrl+, 在 Linux 通过。Windows 宿主当前键盘布局下，Ctrl+, 仅向应用发送 Comma key-up，没有 key-down；SendKeys 与 Win32 VK_CONTROL/OEM_COMMA 复验相同，原始输入保留在 `ctrl-comma-host-input.log`。此环境使用备用快捷键或菜单；未将其报告为 Ctrl+, 通过，也未变更系统输入法设置。

复验新增脚本：`scripts/verify-ui-gpu-windows.ps1`、`scripts/verify-ui-gpu-linux.sh`。测试使用独立 `MDCARD_SETTINGS_PATH`；`MDCARD_RENDERER_REPORT` 写入实际后端信息供审查。Windows UI 自动化应独占测试桌面，Linux 脚本使用专属 Xvfb。下方原有功能验收仍保留，产物哈希以本次交付表与 manifest 为准。

历史主代理独立验收：从 `C:\Windows\Temp` 启动当时发布版，以隔离设置分别验证 Dark/Pink/CPU/620×420/23px 与 Light/Green/GPU/中文路径本地相对图片。两次均取得应用原生截图并正常自动退出，实际渲染器分别为 WARP `Cpu / Dx12` 与 NVIDIA/OpenGL。结果与截图见 `evidence/ui-gpu/master-review/`；其旧 Windows SHA256 为 `13D85BE74B6E1E8A82AEE6388024263D649CC5A1C40B61114116B38D6A5FCF93`，不对应下方本轮语言版本。

## 交付物

| 平台 | 可执行文件 | SHA256 |
| --- | --- | --- |
| Windows x64 GNU | `dist/windows/MDcard.exe` | `8D332E2AA75239CC7A0A8A728AB82A638A3D7BDBE657B0E5CEEB738276222B6D` |
| Linux x64 | `dist/linux/mdcard` | `4C1389CF344F664AA1D0C3F4FE1F398A92406EF2052390BC6DCB62EFE4A6903D` |

打包文件：`dist/MDcard-0.1.0-windows-x64.zip`、`dist/MDcard-0.1.0-linux-x64.tar.gz`，包括可执行文件、示例和许可证。字体嵌入可执行文件，不需要从 `assets` 目录动态读取。完整产物/源码哈希与证据文件名见 `evidence/manifest.json`。

## 原有功能检查（历史）

| 项目 | Windows | Linux | 证据 |
| --- | --- | --- | --- |
| release 构建与本机启动 | 通过；Windows GNU 交叉构建后在 Windows 11 26100 启动，125% DPI | 通过；Debian 12 x64，X11/Xvfb、Mesa 软件渲染 | 两端 GUI 截图、result JSON |
| 单元测试 | 7/7，在 Windows 执行 Rust 测试 exe | 7/7，Linux 原生 cargo test | `evidence/{windows,linux}/unit-tests.log` |
| UTF-8 中文/emoji 编辑→保存→重新打开 | 通过；原生文件对话框、字节完全一致 | 通过；GTK portal，保存后新建，再原生打开 | Windows 最终 run；`evidence/linux/dialog-result.json`、`dialog-saved.sha256`、`chinese-emoji-read.png` |
| Write / Split / Read | 通过 | 通过 | `welcome-native.png` / `read-mode.png` / `images.png` |
| 标题、表格、任务/嵌套列表、引用、代码块、链接 | 通过；主代理从其他 cwd 打开示例 | 通过；从 `/tmp` 打开示例 | `evidence/ui-gpu/master-review/`、`evidence/linux/images.png` |
| 中文与含空格路径的本地相对图片 | 通过 | 通过 | `examples/图片验证.md`、两端图片截图 |
| 新建/退出的未保存 Cancel | 通过；取消后保存内容仍在 | 通过；Escape 取消后保存 | 最终 GUI run 与 Linux `unsaved-dialog.png` |
| Discard / Save changes 继续原动作 | 通过；分别核对旧文件未保存与先保存后新建 | 通用同源码流程；红灯关闭 Discard 实测 | Windows 最终 run 中对应截图和 JSON |
| Save As 成功与取消 | 通过；内容一致、取消不丢修改 | 原生 Save 对话框与保存已实测 | Windows 最终 run、Linux `native-save-dialog.png` |
| 保存失败保留 dirty、外部修改冲突拒绝覆盖 | 通过 | 通过 | 两端相同文档测试 |
| BOM/CRLF 与非法 UTF-8 | 通过；原样保存/编辑 roundtrip；非法编码拒绝 | 通过 | 两端相同文档测试 |
| 切换文档后的 Undo 隔离 | 通过；新文档 Undo 后保存为空 | 文档 generation 使用同源码 | Windows `new-after-undo.md` 和 JSON |
| 小窗口自适应 | 通过；680×480 物理像素，125% DPI 时受最小逻辑尺寸限制 | 通过；680×480 逻辑像素 | 两端 `compact.png` |
| 窗口最大化/还原、最小化/恢复 | 通过；交通灯真实点击和 Win32 状态 | 通过；Openbox 的 EWMH/WM_STATE | `evidence/windows/controls/result.json`、`evidence/linux/*state.txt` |
| 标题拖动、右下角 resize | 通过；原始位置/尺寸记录 | 通过；拖动后连续 resize：1240×820→1300×855 | 两端 controls JSON / `before-drag.txt` / `after-drag.txt` / `after-resize.txt` |
| 红灯关闭的 dirty 保护 | 通过 | 通过 | 两端 dirty-close 截图与 controls JSON |
| 格式与静态检查 | `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过 | Linux 工具链执行，含共享代码 | `evidence/ui-gpu/fmt.log`、`evidence/ui-gpu/clippy.log` |

历史 Windows 全流程 run 为 **`evidence/windows/20261019-074242/`**，对应旧版 exe。Windows 历史控件验收在 `evidence/windows/controls/`，历史独立截图和结果在 `evidence/ui-gpu/master-review/`。本轮最终语言版证据统一位于 `evidence/localization/`。

Linux 原生文件对话框使用隔离 DBus session + GTK portal + Openbox，未修改宿主桌面。容器无 `/dev/fuse`、AT-SPI/PipeWire 服务，portal 日志中有对应警告；实际普通路径文件选择、UTF-8 字节核对、重新打开和干净退出均通过。最终 `dialog-app.log` 与 `controls-app.log` 为空。

## 发现并修复的问题

- 原始分屏源码长行越界、粗黑滚动条、小窗口两列拥挤：修复裁剪、换行、细灰滚动条、分栏间距和自适应视图。
- Windows 125% DPI 的验收脚本截图被裁切：脚本增加线程 DPI awareness，并保留应用原生截图；不是把裁图当成布局通过。
- 原来缺少 emoji 字形：内置 Noto Emoji 并附 OFL；最终中文/🦀 均实际显示。
- Markdown `rust` 代码语言会落入简易高亮 palette：启用 egui_extras syntect fallback，避免浅底荧光绿代码，最终截图使用可读的深色代码文字。
- X11 原生拖动后 egui 的潜在拖动 ID 可能残留：移交窗口移动后清理本地 pointer/drag 状态。Resize 采用独立 32px 交互区与累计位移，已验证连续拖动→resize 流程。
- 早期自动化失败、DPI 裁图及文件名补全 `.md.md` 的旧证据移至忽略的 `artifacts/history/`，不作为最终通过证据。

## 可复验方式

`scripts/verify-windows.ps1` 用真实键鼠、剪贴板和系统文件对话框测试完整文档流程，保留唯一 run 目录并恢复原剪贴板。`scripts/verify-window-controls.ps1` 验证窗口控件。Windows 原生输入法组合输入尚未自动化验证；中文证据为 UTF-8 文件与剪贴板输入，不把它称作输入法兼容性证明。

Linux 三个脚本分别验证主界面、原生对话框与窗口控件：`scripts/verify-linux.sh`、`scripts/verify-linux-dialogs.sh`、`scripts/verify-linux-controls.sh`。在 `Dockerfile` 工具镜像中从 `/work` 执行；截图分别使用隔离 Xvfb display。首次 `cargo build --release --locked` 后再运行脚本。

## 明确边界

- 已验证 Windows 11 x64 与 Debian 12 / X11 / Openbox；不能据此声称所有 Windows/Linux 版本、所有桌面与显示驱动都“完美兼容”。Wayland、GNOME/KDE 真机、其他 DPI/多显示器、Windows/Linux 中文输入法组合输入均需进一步实机验收。
- `.github/workflows/build.yml` 提供 Windows MSVC / Ubuntu 22.04 CI，未推送远端，因此尚无远程 CI 运行证明。
- HTTP(S) 图片 loader 已编译集成；本次图片运行验收覆盖本地图片，未将远程网络可达性作为通过条件。
- 系统 OpenGL/portal 与发行版动态库仍是 Linux 运行依赖；Windows 当前 GNU exe 的导入表只有系统 DLL，见 `evidence/windows/imports.txt`，无需额外分发 MinGW DLL。
- 正常操作有未保存保护；无崩溃/断电自动恢复。原子替换已同步文件，但不将它提升为断电后目录项持久性的承诺。

测试后停止专属 `mdcard-dev` 容器，隔离测试的应用、Xvfb、Openbox、DBus/portal 均随之停止；已有 Dify 容器不受影响。构建镜像、停止的容器与命名 Cargo/target 缓存保留供复验。
