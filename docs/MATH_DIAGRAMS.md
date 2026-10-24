# 数理公式与代码化图表模块：冻结架构与验收

2026-10-19。Markdown 原文始终是权威；预览转换只发生在独立内存副本。

## 架构

保留原生 Rust/egui 编辑器。CommonMark 的数学回调在原有段落、列表、表格布局中绘制内联公式；块公式和图表通过同一回调绘制原生交互节点。受控 Node worker 通过 stdin/stdout JSON 运行本地 Chromium 中的 KaTeX/mhchem、Mermaid、flowchart.js，并调用本地 Java PlantUML。SVG 清洗后才栅格化和导出。不开 HTTP 端口；不将源码传送外部服务。

Rust 根据真实滚动视口筛选节点；250ms 输入防抖；按引擎、内容、主题、字体计算缓存。Chromium 页面使用 IntersectionObserver 确认目标进入渲染画布，但隐藏 worker 的 observer 不作为用户视口懒加载的证据。缓存、解析、提交和渲染计数可供验收。

## 语法和同步契约

支持 `$...$`、`\(...\)`、`$$...$$`、`\[...\]`、math/latex/mermaid/plantuml/puml/flow/flowchart 围栏。用户需求的 `[...]` 解释为标准 LaTeX `\[...\]`；普通 `[链接]` 和方括号内容保持 Markdown。单美元起始后及结束前不能空白，结束美元后不能数字；明确成对的 `$100$` 等纯数字公式受支持；未闭合 `$100` 及常见货币句保留文本。代码、转义美元、围栏内普通文本不再解析。未闭合行内公式保留文字，坏节点局部报错。

节点存储 UTF-8 字节区间和原文快照；源码浮层只替换节点内容，保留定界符、CRLF、周围文字和重复节点身份。变更拒绝陈旧快照，进入原生编辑器的同一撤销链；预览模式、平移、缩放和主题从不写入 Markdown。

## 安全与资源边界

KaTeX trust:false、maxExpand/maxSize；Mermaid securityLevel:strict，禁止用户配置指令/frontmatter；DOMPurify 清洗所有输出，禁止链接、外部资源、脚本、foreignObject。PlantUML SANDBOX，额外拒绝全部预处理/包含/文件/URL 指令。Chromium deny-all 网络、禁 service worker；无可供跨站调用的端口。单节点长度、SVG 尺寸、等待时间和缓存总量均有界。

## 验收矩阵

1. Rust 单元：各定界符/空格/币值/代码上下文，Unicode、CRLF、重复节点、陈旧补丁、源文不变。
2. 真实 Chromium：矩阵、aligned、积分、希腊字母、mhchem；Mermaid flow/sequence/mindmap、PlantUML flow/sequence/mindmap、flowchart.js flow。SVG 和 PNG 有效且有真实图元。
3. 恶意输入：TeX HTML/URL、Mermaid 配置/XSS、SVG 外部引用、PlantUML include/网络，超长输入、错误后继续有效渲染。
4. 桌面：行内同段排版、点击源码、250ms 同步、统一 undo/redo、主题/预览不改源、模式/缩放/平移/导出，Windows 和 Linux 各自运行证据。
5. 性能：未变节点零 engine render、同内容复用、视口外零提交；报告真实耗时和平台。60fps 仅在测量支持的场景声明；不将缓存命中外推为全编辑器 60fps。

KaTeX 的支持语法有边界，不声明无限 TeX 的 100% 覆盖；安全样例通过也不声明所有未来 XSS 均不可能。平台与发布结论以新鲜证据为准。

## 实现细节与可复验入口（2026-10-20）

`src/rich_nodes.rs` 使用 Markdown AST 文本范围排除代码、图片、链接目标等上下文；引用/列表围栏保留每行容器前缀和 CR、LF、CRLF，不对全文统一换行。源码浮层补丁验证全文版本、节点原文及新包装范围，拒绝通过新增定界符移动到相邻节点。渲染错误不会替换整篇内容。

`src/rich_ui.rs` 缓存解码像素与 SVG，通过内容+主题+字号标识复用；未修改节点不重新提交引擎。Rust 用户视口是懒加载依据；CommonMark 顶层块按实际高度缓存和视口跳过，含全局引用定义的文档保留完整解析路径。`src/rich_svg.rs` 使用后台有界队列解析和按缩放重栅格 SVG，像素 LRU 总预算 64MiB，单图 8MP/4096px；缩放期间先显示上一份有效 PNG，不会重跑 JS 引擎。

`src/source_layout.rs` 与 egui 原生布局逐行对照，缓存段落换行标记并共享未变 glyph/mesh，避免 `Galley::concat` 在每次打字时复制全部段落的末行。保留完整原文、字符坐标、选区和原生 undoer。侧栏大纲防抖 250ms。

Renderer 缓存总预算 32MiB/128 项，Rust 节点结果约 64MiB/128 项；单源码节点 64KiB；Java 192MiB/6s，浏览器单次 8s，Rust supervisor 18s（包含管道阻塞写），队列 4，关闭和超时终止子进程树。Java SANDBOX 是额外防线，预处理/外部文件与网络指令在调用前拒绝。Chromium 网络请求/WebSocket 拒绝；以 root 运行的 Linux Docker 验收因 Chromium 限制关闭其 OS sandbox，普通用户/Windows 保留 sandbox。建议正常桌面以普通用户运行。

SVG 先 DOMPurify，再用 CSSOM 白名单与计算样式固化必要图形属性，再次清洗；SVG 不含外链、可执行元素或用户 CSS。导出默认插入可信主题背景，独立白底查看器也保持暗色可读。内嵌 SC/KR 静态 OFL 字体及 fallback 提供中文、日文、韩文标签；字体经修改已使用新名称。

运行脚本：

- `renderer/test.mjs`：真实引擎、亮暗主题、缓存和错误恢复。
- `scripts/verify-rich-independent.mjs`：独立引擎/攻击/CSS/SVG 箭头/IPC 验收。
- `tests/rich-acceptance`：独立 parser、原子补丁、随机畸形输入。
- `scripts/verify-rich-windows.ps1 -OutputDir <dir> -BomCrLf`：真实 Windows GUI、BOM/CRLF、编辑/撤销、模式、缩放/平移/导出。
- `scripts/verify-rich-lifecycle.ps1 -Exe <exe>`：假 worker 不读 stdin、超时重启和退出。
- `scripts/verify-rich-linux.py`：私有 Xvfb/Openbox 真实键鼠、六语、视口懒加载与连续输入性能。`MDCARD_RICH_OUTPUT` / `MDCARD_RICH_EXE` / `MDCARD_RICH_DISPLAY` 可隔离测试。

`MDCARD_RICH_REPORT` 仅验收启用，报告 parse/submission/cache/visible/offscreen/completed/error、实际 viewStates、控件区域、输入帧/滚动帧/交互帧 update CPU 数组与 sourceEditor phase。CPU 不等于 GPU 呈现帧率；输入测试必须断言保存字符完整，不能混入大量 idle 帧宣称 60fps。

官方参考：[KaTeX 支持语法](https://katex.org/docs/supported.html)、[KaTeX 安全](https://katex.org/docs/security.html)、[Mermaid 配置](https://mermaid.js.org/config/schema-docs/config.html)、[Mermaid 安全公告](https://github.com/mermaid-js/mermaid/security/advisories)、[PlantUML 安全](https://plantuml.com/security)、[DOMPurify](https://github.com/cure53/DOMPurify)、[egui TextEdit](https://docs.rs/egui/0.33.3/egui/widgets/text_edit/struct.TextEdit.html)。

## 最终本地验收记录

最终原生程序：Windows `F61C4C6312EB760C6F98581752E24D73BDB92941C91BB1C0A8BC3E4D00AE434F`；Linux `05b7b41683225993f77cc2c07886f1a73a7c7d6d3f1e6d38629f81c28f1c8580`（SHA256）。最终功能报告：`evidence/rich/windows-final/result.json`、`evidence/rich/linux-release/result.json`。早期 `linux` / `linux-final` / `linux-performance` / `windows` / `windows-debug` 是开发记录，不能覆盖最终结果。独立验收见 `evidence/rich/master-review`。

Windows 最终GUI验证公式同行、节点源码编辑、真正左右分屏、共享撤销/重做、滚轮缩放与拖拽平移、SVG/PNG 导出、亮暗主题、未改节点零新提交及五秒内退出。分屏源码列与画布位置还有几何断言。Linux 最终GUI验证同一编辑/历史/缩放路径、六语菜单、韩文原生SVG标签；40个图表初始仅提交4个，滚动后累计17个，原文无改变。Linux 导出对话框未另测，以 Windows 的真实导出为证据。

性能平台为 Debian12 / Xvfb X11 / Openbox / llvmpipe，Node24 / Chromium155 / Java17。两种文档分别在 Split 和 Write 连续输入104字符（60ms键间隔），每份最终保存208字符，逐字对照通过。统计只取真实输入帧和滚动帧；交互包含输入后500ms内防抖解析，GPU呈现和系统输入传输不在计时内。

| 文档 | Split 输入 P95 / 最大 | Write 输入 P95 / 最大 | Split / Write 滚动 P95 |
| --- | --- | --- | --- |
| 150,895B，2,000公式节点 | 4.11 / 4.76ms | 3.09 / 4.30ms | 2.05 / 1.68ms |
| 1,141,895B（1.09MiB），5,000公式节点 | 15.58 / 20.41ms | 14.04 / 16.85ms | 7.82 / 6.01ms |

该1MiB样本的 P95 输入CPU落在16.7ms预算内，但部分输入帧超预算，防抖后的交互最大30.82ms；小文档冷启动/首次渲染也有27.51ms update。**没有实测GPU连续呈现60fps，也不宣称任意长文档、任意语法或每一帧无掉帧。** 引擎超时、冷字体加载、大型布局和含全局引用定义文档可能更慢。完整原始数组和运行UUID、程序hash在对应报告中。

包清单与哈希由 `scripts/package-rich.py` 写入 `evidence/rich/packages/result.json`；独立逐文件归档和安全路径校验为 `scripts/verify-rich-packages.py`，Linux 干净解压原生启动为 `scripts/verify-rich-package-linux.py`；Windows干净包可用 `verify-rich-windows.ps1 -UsePackagedRenderer -BomCrLf -Exe <解压MDcard.exe>`。这些发行包包含本地 Node 和引擎；系统浏览器与 Java 是明确前置依赖，禁止把开发目录启动证据当成干净包启动证据。

SVG 保留文字与字体族；外部查看器通过对应字体或系统fallback显示文字，不自动安装字体。包内OFL字体与许可证在renderer/vendor/fonts和根目录通知文件中。
