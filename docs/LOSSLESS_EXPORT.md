# 多格式导出引擎：冻结契约与验收矩阵

2026-10-22 后续审计更新：取消与超时需等待已跟踪进程终态和输出管道关闭。Linux 原生导出在启动前检查 pidfd API（内核 5.3+ 及相应权限）；不满足时明确拒绝启动。清理未确认或发布已开始时，界面要求核查目标，不能声称旧目标必然保留。普通取消、失败保护、重试与平台边界见 [后续台账](BUG_AUDIT_FOLLOWUP_2026-10-22.md)。本节其余历史验收不能替代新候选包证据。

## 契约（2026-10-20）

编辑器在启动任务时复制当前内存文档和样式；导出不改源文件、不触发保存、不丢失焦点或选区。Rust UI 仅接收后台进度事件和取消操作。每个任务使用独立子进程和私有临时目录，输出先写入目标同目录临时文件，成功校验后原子替换；失败、超时和取消保留原有目标。错误含阶段、行号/资源地址。

“内置原生管线”指原生 Rust 应用调度随包分发的 Node、Chromium 兼容 Chrome for Testing、JRE 和锁定 JS 库，不要求用户安装系统 Chrome、Node、Java、Pandoc 或 Marp。它不是纯 Rust 排版。HTML/PDF/PNG/JPEG/Marp HTML/PDF/PPTX 使用此管线。Pandoc 是独立可配置管线，支持 DOCX、EPUB；缺失时给出可配置路径和官方下载引导，绝不静默更换文件格式。DOCX/EPUB 采用 Pandoc 的原生标题/目录/元数据结构。

渲染遵循 Markdown AST；公式转换为 MathJax SVG，Mermaid/PlantUML/FlowChart 转换为 SVG，图像及 CSS 资源递归预编译为内联数据。不可解析资源、脚本/危险原始 HTML、SVG 外部引用和导入样式阻止导出并定位。离线产物不再运行公式或图表编译器。打印分页支持 `<!-- pagebreak -->`、`\newpage` 和 CSS `.pagebreak`。代码/表格/图片具有 avoid 规则；单块超过打印区域时拒绝并定位，避免无提示截断。电子书保留分页提示，物理页数由阅读器决定。DOCX TOC 是可点击的已填充目录并含更新字段，最终字体和分页仍由 Word 排版引擎与 Reference Docx 决定，不能跨引擎承诺逐像素相同。

主题可选择编辑器当前浅/深色、纸张、字号、代码行号、页边距和自定义 CSS。代码使用 highlight.js 多语言词法高亮。高清长图按受控 tile 截图并拼接，超过像素预算显式拒绝。资源单个限 20 MiB，远程资源有超时，任务总时长有上限；取消终止子进程树。

## 验收矩阵（实现前冻结）

| ID | 验收项 | 必须检查的真实产物/行为 |
|---|---|---|
| EX-01 | 内置 HTML/PDF/PNG/JPEG | 禁系统浏览器/Java/Node PATH 后运行；离线 HTML、有效 PDF、完整高清长图 |
| EX-02 | Pandoc DOCX/EPUB | 正常和自选 Reference Docx、EPUB title/author/language/identifier、目录层级与媒体容器 |
| EX-03 | Marp 三格式 | 真实分隔页/指令、HTML section、PDF 页面、PPTX 幻灯片媒体 |
| EX-04 | 结构/样式 | PDF 嵌套 Bookmarks、Word TOC、EPUB nav、多语言行号高亮、背景、显式分页 |
| EX-05 | 资源预编译 | 数学与三种图表、相对本地图与 HTTP 图；所有产物离线打开无源码或裂图 |
| EX-06 | 严格完整性 | 无效图片、CSS 导入、SVG 外链、超高块、超时明确定位；失败不覆盖旧文件 |
| EX-07 | 异步与取消 | 大文档 UI 持续响应、阶段进度、取消子进程树与临时文件清理，旧目标不变 |
| EX-08 | 产品和交付 | 六国语言导出窗口、工具配置/缺失引导、Windows/Linux 随包运行时及哈希清单 |

每项需新鲜自动化或人工证据；依赖准备不等于测试通过。主代理独立验收。

## 使用

打开 Markdown，选择工具栏 `文件 → 导出…`，或 `Ctrl+Alt+E`。导出绑定打开窗口时的主/副窗格身份，窗口显示源路径；点击导出时复制该文档当前内存内容。文档可继续编辑。选择格式、当前编辑器/浅色/深色主题、A4/Letter、字号、页边距、高清倍率、目录、行号；配置区提供 Reference DOCX、EPUB 元数据、自定义 CSS 和 Pandoc 路径。自定义 CSS 作用于 HTML/PDF/长图/EPUB/Marp，Word 使用其原生样式和 Reference DOCX。指定 Reference DOCX 时，模板的正文/标题/页型/页边距/页眉优先于编辑器默认主题参数；代码的高亮与防拆分语义仍应用。

任务窗口显示阶段和进度，`取消导出` 终止整个转换子进程树，后台删除私有工作目录；正常关闭应用会等待任务回收。导出不能覆盖任一已打开文档，也不能覆盖命令行声明的 sourcePath。目标同名旧文件只在新产物全部渲染/校验成功后原子替换。

三种分页写法：独立一行的 `<!-- pagebreak -->`、独立段落的 `\newpage`，或原始 HTML 的 `<div class="pagebreak"></div>`。代码块/转义上下文不会被误当指令。PDF 真正分页并嵌套书签；DOCX 具有原生分页符、立即可点击的缓存目录和可更新 TOC 字段；EPUB 为可重排电子书，nav 保留标题层级，阅读器决定最终物理页数。

MathJax 严格编译 TeX，三种图表复用编辑器的真实引擎。图片解析以源文件所在目录为基准（未保存文档使用工作目录），支持相对文件、data URI、HTTP/HTTPS；资源先下载并校验再嵌入，产物离线不执行编译代码。PlantUML 使用 JRE 与 Smetana Java 布局，不要求 Graphviz。DOCX 同时包含可引用 SVG 和高清 PNG fallback，正文嵌入许可字体；Marp PPTX 遵循 Marp CLI 的高清图片式幻灯片，文本不具备 PowerPoint 原生可编辑性。

字体通过随包 HarfBuzz/WASM 按实际文字子集化：正文 HTML/Marp 包含 WOFF2，EPUB 显式打包到字体目录及清单，DOCX 采用原生混淆 SFNT 嵌入（韩文使用 KR 字体）。每张图表 SVG 自身嵌入标签所需字体，作为独立图片也不依赖父文档 CSS 或系统中日韩字体。独立 SVG 页面阻断网络后的三引擎/混合中文韩文俄文验证入口为 `node scripts/verify-export-svg-fonts.mjs`。

严格拒绝失效资源、活动 HTML/SVG、SVG 外部引用、CSS `@import` 和 CSS 转义，报告阶段和源行号。图片/代码/表格超过一页时 PDF 拒绝输出，要求拆分或减小尺寸；Word 和电子书由各自排版引擎决定可重排结构。完整性优先于发布半成品。长图上限 100MP/60000px、单资源 20MiB、资源合计 128MiB、默认任务截止 180 秒；每个资源请求还受 12 秒限制。

## 重建、集成与自动化

```powershell
cd renderer
# 锁定的 Sharp 双架构依赖需显式 --force，避免 npm 丢弃另一平台二进制。
npm ci --force --ignore-scripts --no-fund --no-audit
node build.mjs
node licenses.mjs
cd ..
python scripts/prepare-export-runtime.py --proxy http://127.0.0.1:7897
# Linux/Windows 分别构建原生二进制后：
node scripts/verify-export.mjs
node scripts/verify-export-svg-fonts.mjs
python scripts/package-export.py --windows-exe artifacts/export/MDcard.exe --linux-exe artifacts/export/mdcard
# 对完整发行 stage 以清空 PATH 的子进程再验收九格式：
node scripts/verify-export-packages.mjs dist/export/<timestamp>/windows
```

运行时版本/官方来源/精确 SHA256 位于 `renderer/export-runtime-sources.json`，npm 在 package-lock.json 锁定。JRE SHA256 来自 Adoptium 官方元数据；Chrome for Testing 和 Pandoc 的首次下载采用官方 HTTPS，保存实测 SHA256，随后重建严格校验。该差异明确写入来源清单，不把实测哈希冒充上游签名。归档提取检查路径、类型和内部链接，避免跨目录解压；Linux 桌面仍需要 glibc 2.36+ 与常规桌面系统库（libnss3/libatk/libxkbcommon/libgbm/libasound 等），没有要求安装浏览器或 Java 应用。

CLI 与 GUI 使用相同引擎：`renderer/runtime/node.exe renderer/export.mjs request.json`（Linux 为 `node`）。JSON 结构：

```json
{"source":"# A\n\nText","sourcePath":"D:/notes/a.md","baseDir":"D:/notes","output":"D:/exports/a.pdf","format":"pdf","timeoutMs":180000,"options":{"dark":false,"paper":"A4","marginMm":18,"fontSize":16,"scale":2,"lineNumbers":true,"toc":true,"pandocPath":"D:/tools/pandoc.exe","referenceDocx":"D:/templates/reference.docx","title":"Book","author":"Author","language":"zh-CN","identifier":"urn:uuid:example","customCss":"h1{letter-spacing:.02em}"}}
```

`format`：`html/pdf/png/jpeg/docx/epub/marp-html/marp-pdf/marp-pptx`。stdout 是 progress/complete/error JSONL；成功包含输出字节数、SHA256、标题结构、资源数量、耗时；错误包含 stage/message/line/resource。`MDCARD_EXPORT_REPORT` 提供只观察的 UI 控件位置、任务状态/进度/错误/输出、源快照与当前内容哈希，用于真实键鼠验收，不触发导出。
