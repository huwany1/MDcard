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

## 回归入口

`python scripts/check.py --renderer` 执行真实引擎主题/缓存/错误恢复和进程/CLI 回归。`node scripts/verify-rich-independent.mjs` 检查清洗后 SVG 的真实样式、箭头、攻击与 IPC；`tests/rich-acceptance` 直接引入真实 parser/补丁代码，覆盖随机畸形输入。前提和额外 GUI/导出工具见[开发说明](DEVELOPMENT.md)与[工具入口](../scripts/README.md)。

ELK 布局不包含在当前 browser bundle，指定它会明确报错；默认 Dagre、sequence、mindmap 继续使用。平台、DPI、输入法与端到端性能需要独立实际验证。
