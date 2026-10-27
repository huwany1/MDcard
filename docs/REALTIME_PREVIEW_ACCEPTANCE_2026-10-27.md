# 2026-10-27 实时预览验收

状态：下列内容记录冻结R7的Windows本地验收；冻结验收时尚未推送或更新发行包。随后本地整理与PR状态见文末“本地整理与历史二进制归档”。性能数据仍只代表这里列出的原始二进制、样本和观测边界。

## 最终产物与来源

- 开发分支 `codex/continuous-preview`，基线 `main@38a97646f9ad56dee6a77b4110e88bdd2d5c5dad`；未修改用户原有 `docs/ui-ux-proposal-2026-10-27/`。
- 最终候选 `artifacts/preview-2026-10-27/candidate-r7-final/mdcard.exe`，SHA256 `afbe767cfdfeb4ec6ee81f0c5881e707ac1490e93dcfca0e3fc4be335167008d`。
- 同目录 `candidate-manifest.json` 保存全部运行时源码文件哈希；指纹 `c1ac8d9c21e9bdd1b5e3c0a874d3c8797f09cf8f0531c09f97b54acf3d008b50`。构建来源 `evidence/preview/2026-10-27/final-r7-regression-build.log`；独立 target `/work/target/preview-candidate-r3`。
- 最终根测试 290 通过、0失败、6忽略，Clippy `--all-targets -- -D warnings` 通过。新增离屏粘贴回归在修复前真实失败，修复后通过；像素锚回归覆盖180和900逻辑像素，180档明确断言实际发生非零高度补偿。
- 主代理另运行 `cargo test --locked preview`，20/20通过；日志 `master-final-preview-tests.log`。主代理也目视复核真正中段两图的同Paragraph148451与Y位置。

所有下述证据路径相对 `evidence/preview/2026-10-27/`。每个原生场景包含 `screen.mp4`、首次打开录屏、动作时间、逐帧捕获时间、UI/reading/rich报告、设置、实际exe哈希和 `outcome.json`。原始失败目录保留。

## 根因和修复

1. 原 `main::preview` 把“可显示同文档快照”和“当前源码坐标可用”混为一谈，小文档250ms防抖每次输入都清空正文画spinner。现在独立检查 `displayable` 与 `source_ready`；只在首次无本稿件快照时显示首次准备状态。
2. 大文档只接收恰好等于最新revision的结果，持续输入时完成的中间版本会被丢弃。现在同epoch中单调接收已完成版本，旧版本仅供显示；reading plan、geometry、校准、阅读写入、双栏坐标同步和富媒体源码写回仍要求当前版本。
3. 小文档改为120ms空闲防抖/300ms最长等待；Rope worker保留一个运行任务、一个最新排队快照和一个结果，累计InputEdit最多256。解析/转换缓存、媒体队列和图片LRU均有上限，详见 [实现说明](REALTIME_PREVIEW.md)。
4. 旧阅读记录的近似恢复曾覆盖内容key像素锚。没有可靠relocation proof时，当前buffer/generation的像素锚保持到当前index和实际绘制geometry就绪。
5. 离屏Paste原来把内部源码显露写到通用 `jump`，被当作用户主动导航而取消预览锚。现在 `source_reveal` 与TOC/搜索/导航键分离；两栏offset只有真实滚轮/拖动手势才相互驱动，内部排版/光标显露不来回拉扯。
6. 近页首改块的估算高度在实际排版后还会变化。R6逐帧trace证明：hold246.22、实际delta−49、校正后197.22，却在该帧捕获校正前geometry并释放hold，下一帧被错误geometry拉到247.22。现在hold随delta补偿，该帧不确认geometry，等待校正offset真正绘制后再释放。没有移除版本保护。
7. 目录展示和跳转消费都检查文档/generation/源码及parse版本，验证字节边界，拒绝旧尾部heading。图片160逻辑像素局部占位、15秒超时说明/Retry；公式和图表错误卡片也提供重试。移除了全局250ms唤醒，各子系统安排自身唤醒。
8. 去除 `layout_blocks` 为被丢弃返回值完整clone全部块的开销。1MiB样本源码编辑响应尾部收敛，数据见下。

## Windows 原生验收

| 场景 | 最终证据与结果 |
| --- | --- |
| 原始白屏复现 | `baseline-small-real/during-input.png` 与录屏：源码增加、预览正文消失只余spinner；后续基线连续组同样复现 |
| ≥30秒连续输入 | `final-r7-suite/00-continuous`：真实动作跨度31.890秒，489/489正文捕获帧可读，0空白/spinner-only；164次预览提交，提交间隔P50/P95/max=180.7/289.8/590.1ms。最后一次连续输入至当前版本报告上界196.9ms；最终mapped revision177，保存正文包含FINAL-CONSISTENT |
| 真实中文IME/删除/粘贴/撤销重做 | `final-r7-suite/01-ime`：Microsoft拼音候选条与preedit/commit真实事件；关键帧仍有可读正文。各场景后续均执行原生delete/undo/redo/final paste，无重建源码TextEdit |
| 16MiB完整矩阵 | `final-r7-suite/05-matrix-16m`：16,776,192字节；20个真实键、273,015字节粘贴、undo/redo/undo、预览滚轮、Read→Split→Write→Split、4档窗口宽度、降至39字节/undo恢复、4次文件打开均通过。可见byte313→333，Y14.59375不变；更新前后源码焦点Some(7EAE)、选区[146,146]不变 |
| 真正中段的离屏光标编辑 | `final-r7-anchor-middle`：真实拖预览滚动条至约8MiB位置，源码caret仍在前部[106,106]；Paste20字节后可见byte8,461,772→8,461,792，Y15.0→15.0。输入后与预览完成后选区均[126,126]、焦点Some(7EAE)，没有主动源码导航 |
| 图片等待/错误/超时与恢复 | `final-r7-media`：原始hang请求25.06秒后返回，16.64秒时已通过真实Retry发起第二次请求并立即成功；invalid也通过第二请求成功。实际placeholder_height=160；等待时真实源码编辑成功，正文持续可读 |
| 公式/图表失败与重试 | 同目录错误说明、Retry按钮、第二次公式请求和修复源码后的两条ok=true；有效公式render154.2ms、Mermaid421.7ms，含队列/启动的完成时间分别604.1/1026.0ms。冷启动和失败不是纯render耗时 |
| 默认缩放及混合内容 | `final-r7-suite/03-rich/default-zoom-formula-diagram.png`：新实例zoom1下E=mc²、积分1/3及Start→Ready清晰可读；`table-long-code.png`含中文表格、250行长代码原生滚动 |
| 自动保存/恢复/搜索 | `final-native-regressions/result.json` 全true：自动保存自身定时、显式关闭autosave后独立恢复定时、手动保存、搜索选中源码、杀进程/重启发现恢复记录、精确恢复独立脏副本、原文件哈希不变。整段录屏为 `final-native-regressions-screen.mp4` |
| 资源有界与闲置 | `final-r7-suite/02-resources`：100图片轮换、24次原生文件切换、102HTTP/101独立URI；首URI第二次请求证明旧loader已淘汰。所有6个最终suite场景闲置4秒frame均不增 |

主代理独立复核了基线白屏、最终哈希、完整矩阵的位置/焦点/选区、默认zoom1公式图表、表格长代码和精确恢复副本，未仅依赖子代理的通过声明。

## 首次加载与巨型粘贴的独立样本

`startup-giant-paste-timings.json` 从冻结R7的 `final-r7-suite/05-matrix-16m` 原件导出。两个场景各n=1，不能据此估计P95；首次加载指新进程首次打开16,776,192字节文件，未控制OS文件缓存，不能称文件系统冷缓存。

首次加载的外部起点是 `environment.launch_started=30887.6280933` 秒（调用subprocess.Popen前的Python perf_counter，Windows QPC）。Rust `ui_draw_clock_ms` 同样使用QPC。以下完成上界包含报告/捕获延迟：

| 首次加载观测 | n | 自上述起点ms | 边界 |
| --- | --- | --- | --- |
| 首个文档路径/UI报告 | 1 | 376.01 | 仅证明UI已有文档；不等于源码可交互 |
| 首个current预览UI命令 | 1 | 2959.68 | buffer2 / generation1 / revision0，已经产生绘制命令；不等于GL/swap完成 |
| 首个显示revision0的launch报告 | 1 | 2973.41 | 诊断发布观察上界 |
| 首个保留的可读桌面帧 | 1 | 4535.06 | screen.mp4第0帧；源码/预览正文均可读，属于宽松观察上界，未据此宣称首次出现恰在4.54秒 |
| 真实源码输入已响应的确认上界 | 1 | 5611.26 | 第一次保留的源码字节增加报告；包含驱动主动等待窗口、焦点及1秒等待，不能当应用最早可交互延迟 |
| 最早源码可交互更新时刻 | 0 | 未采 | 初始每次打开的performance计数已被后续文件重开覆盖，launch采样没有laid_out_lines/source-ready字段，不能补造 |
| 首次预览对应GL完成 | 0 | 未采 | 没有带预览revision的非输入GL时间戳；物理面板呈现也未采 |

初次预览的单次worker parse=254.93ms、mapping/transform=2324.77ms、防抖0ms，预览入口至UI命令=134.23ms（包含ensure/install，不能作为纯layout，不能与worker阶段相加）。纯源码Editor阶段和独立初次块布局没有保留按该次打开关联的分布。最终 `performance.json` 中source interactive446.37ms、worker preview update2477.22ms属于generation5的末次重开，明确不用于首次加载。其132.83ms generic first GL callback还可能绘制加载状态，时钟从首个LargeDocument.opened开始且没有保留绝对起点，也不能当作首次预览。

巨型粘贴的起点是 `actions.giant-paste.at=30895.5117601` 秒（原生Ctrl+V注入之前）；20个真实键完成后，16,776,212字节增加273,015字节，得到17,049,227字节，实际目标为buffer2 / generation1 / revision19。该次巨型粘贴不属于≤1MiB局部8字节编辑的500ms目标。

| 巨型粘贴观测 | n | 自Paste动作ms | 边界 |
| --- | --- | --- | --- |
| revision19源码输入帧GL callback | 1 | 66.49 | 源码响应；该帧仍可显示旧预览，不是新预览完成 |
| exact bytes/revision19源码报告 | 1 | 89.83 | 源码17,049,227字节已经安装 |
| revision19 current预览UI命令 | 1 | 3506.67 | 精确对应Paste版本；不是后续undo版本 |
| 当前预览报告上界 | 1 | 3558.59 | UI revision19且mappedRevision=[2,19] |
| 当前版本桌面观察上界 | 1 | 3601.94 | screen.mp4第116帧明确出现GIANT PASTE；116–120帧预览像素哈希一致，且早于首次观察到revision20的撤销源码报告 |
| current预览对应GL完成 | 0 | 未采 | 输入帧GL有revision/QPC；后来提交当前预览的非输入帧没有单独关联记录 |
| 纯源码Editor阶段 | 0 | 未采 | SourceEditor数组没有该Paste的revision标记，不猜配某个值 |

该Paste的单次worker parse=201.50ms、mapping/transform=1510.70ms、防抖0ms；预览入口至UI命令145.07ms同样不是纯layout。总完成时间包含此前尚在运行的中间版本与队列等待，不减去这些实际等待。首个后续源码revision20报告为30899.3349069秒，正文已回到16,776,212字节；上述current UI与第116帧均早于它，未拿undo之后的完成时间替代Paste。

两张提取帧为 `final-r7-suite/05-matrix-16m/startup-first-recorded-preview.png` 与 `giant-paste-current-preview.png`；帧与动作的真实时间来自frames.json/actions.json，而非录屏名义20fps推断的时间。

## 性能环境与统计口径

Intel Core i7-13650HX（14核/20线程）、约16GiB RAM、Windows11 build26100、NVIDIA RTX4060 Laptop GPU，驱动32.0.15.9608；1550×1024物理像素，125%DPI，Light、15号字、减少动效、OpenGL。GPU枚举原件为 `gpu.json`，没有把枚举刷新率当作逐帧呈现测量。Rust1.92，release opt-level2/strip，WindowsGNU。在正式采样期间不编译或跑测试，未剔除慢样本。

固定大小局部编辑在同一段落替换8字节槽，每次记录动作前revision/选区、expected revision及首个新版本实际字节数。五档基线与R4候选均40次，每档40次大小断言全匹配，桌面观察无缺失/截尾。原件路径和所有统计在 `fixed-slot-comparison.json`。

- 基线普通安装行为exe为 `baseline/mdcard.exe`；固定槽比较使用只增加计时字段的 `baseline-measured/mdcard.exe`，其 `baseline-instrumentation.diff` 已独立复核无调度改变。
- 完整40次矩阵测量候选是R4，SHA256 `edf3e580e94c74c456ba8fcd05af569d17de224914a92e19355139b6eed96b85`，目录 `r4-fixed-suite`。 实际已测exe仍保留在 `D:\MDcard\artifacts\preview-2026-10-27\candidate-r4\mdcard.exe`；另保存只读冻结副本 `D:\MDcard\artifacts\preview-2026-10-27\frozen-sha256\edf3e580e94c74c456ba8fcd05af569d17de224914a92e19355139b6eed96b85\mdcard.exe`，91,391,488字节，重算SHA256完全一致。只读属性防止误覆盖，内容身份仍以哈希为准。后来R5–R7修正滚动意图与像素锚帧顺序，未重跑全部五档40次，不能把R4原件标成最终exe。
- 冻结R7在1MiB−1另做12次代表性复测：桌面P50/P95/max=201.2/325.5/325.5ms，0缺失；源码报告83.95/199.75/199.75ms。其余功能录屏全部使用冻结R7。
- 桌面时延是“首次与该编辑最终稳定正文像素完全一致”的捕获完成上界，关联对应源码revision；spinner像素变化不算。它包含采集、编码提交/摘要和诊断发布延迟，不是物理面板光子延迟。
- UI命令提交、GL callback与桌面观察分别报告。GL默认没有强制GPU fence，也不是swap/scanout；小文档路径没有对应源码GL动作关联数据。
- 所有Instant.elapsed是wall time，历史字段名含cpu也不代表线程CPU。`layout_ui_ms`从预览入口计时，包含ensure/同步解析，不能当纯layout、不能与parse相加。纯块布局来自reading metrics，样本单位是块调用而非40次编辑。
- 录屏按名义20fps编码；真实持续时间读取动作与逐帧捕获时间，不能从视频帧数推断。

下表均为毫秒，单元格顺序P50 / P95 / max。

| UTF-8字节数 | 基线桌面观察 | R4桌面观察 | 样本 |
| --- | --- | --- | --- |
| 8,192 | 352.94 / 434.18 / 448.82 | 235.41 / 351.18 / 568.96 | 40 / 40，0缺失 |
| 65,535 | 365.17 / 416.36 / 484.07 | 245.86 / 342.78 / 366.78 | 40 / 40，0缺失 |
| 65,537 | 120.49 / 217.83 / 455.00 | 121.80 / 200.97 / 346.51 | 40 / 40，0缺失 |
| 1,048,575 | 207.95 / 246.83 / 289.65 | 198.35 / 242.23 / 412.11 | 40 / 40，0缺失 |
| 1,048,577 | 225.25 / 294.25 / 378.33 | 196.41 / 328.48 / 405.41 | 40 / 40，0缺失 |

五档R4桌面P95均≤500ms，8KiB最大值568.96ms仍保留，不宣称每次编辑都≤500ms。1MiB−1基线/候选源码动作至GL callback P95为56.76/56.86ms；输入帧wall P95为3.97/4.14ms。未靠阻塞编辑等待预览降低时延，改善主要在预览持续可读及小文档等待，而不是声称所有大文档解析都显著更快。

| 字节数 | R4源码报告上界 | R4 UI命令 | R4源码动作→GL callback |
| --- | --- | --- | --- |
| 8,192 | 77.91 / 121.96 / 175.20 | 143.57 / 201.20 / 300.42 | — |
| 65,535 | 68.83 / 147.24 / 190.83 | 151.12 / 259.46 / 295.68 | — |
| 65,537 | 74.20 / 145.35 / 303.86 | 26.87 / 97.39 / 267.12 | 23.60 / 95.08 / 264.88 |
| 1,048,575 | 83.57 / 115.96 / 193.92 | 101.42 / 141.95 / 323.63 | 27.13 / 56.86 / 122.87 |
| 1,048,577 | 81.71 / 216.69 / 279.91 | 108.71 / 216.66 / 290.60 | 28.12 / 95.97 / 174.01 |

## 阶段分解

候选每档40次、各阶段P50/P95/max如下。普通文档的debounce是实际等待wall；worker无额外debounce记0。独立worker转换包含Markdown与映射转换，不是纯语法解析。

| 字节数 | 阶段 | P50 / P95 / max ms | n |
| --- | --- | --- | --- |
| 8,192 | 防抖等待 | 123.91 / 148.22 / 190.55 | 40 |
| 8,192 | 解析 | 0.15 / 0.22 / 0.26 | 40 |
| 8,192 | 映射/转换 | 0.07 / 0.12 / 0.18 | 40 |
| 8,192 | 预览入口至UI命令（包含ensure） | 0.70 / 1.05 / 1.14 | 40 |
| 65,535 | 防抖等待 | 123.85 / 171.11 / 258.23 | 40 |
| 65,535 | 解析 | 1.40 / 1.80 / 1.95 | 40 |
| 65,535 | 映射/转换 | 0.54 / 0.81 / 0.86 | 40 |
| 65,535 | 预览入口至UI命令（包含ensure） | 2.54 / 3.52 / 3.71 | 40 |
| 65,537 | 防抖等待 | 0.00 / 0.00 / 0.00 | 40 |
| 65,537 | 解析 | 0.08 / 0.13 / 0.14 | 40 |
| 65,537 | 映射/转换 | 3.11 / 4.54 / 5.11 | 40 |
| 65,537 | 预览入口至UI命令（包含ensure） | 0.69 / 1.01 / 1.18 | 40 |
| 1,048,575 | 防抖等待 | 0.00 / 0.00 / 0.00 | 40 |
| 1,048,575 | 解析 | 0.14 / 0.21 / 0.26 | 40 |
| 1,048,575 | 映射/转换 | 64.21 / 77.50 / 82.53 | 40 |
| 1,048,575 | 预览入口至UI命令（包含ensure） | 3.87 / 5.28 / 5.47 | 40 |
| 1,048,577 | 防抖等待 | 0.00 / 0.00 / 0.00 | 40 |
| 1,048,577 | 解析 | 0.13 / 0.23 / 0.31 | 40 |
| 1,048,577 | 映射/转换 | 65.34 / 77.32 / 79.50 | 40 |
| 1,048,577 | 预览入口至UI命令（包含ensure） | 3.99 / 5.87 / 6.73 | 40 |

基线可对应动作的阶段如下；≥64KiB的parse来自worker parse_records。基线普通parser的baselineMappingMs字段在大文档路径可能为旧值/0，已排除，不能解释为大文档转换零耗时。基线大文档缺少逐动作transform分布，不补造数据。基线小文档250ms等待策略来自源码；没有独立动作相关debounce测量。

| 字节数 | 基线parse | 基线普通mapping |
| --- | --- | --- |
| 8,192 | 0.15 / 0.22 / 0.27 | 0.07 / 0.11 / 0.38 |
| 65,535 | 1.37 / 1.82 / 2.01 | 0.55 / 0.80 / 0.92 |
| 65,537 | 0.09 / 0.15 / 0.16 | — |
| 1,048,575 | 0.14 / 0.22 / 0.33 | — |
| 1,048,577 | 0.14 / 0.28 / 0.33 | — |

1MiB−1 R4的独立布局窗口样本：

| 调用 | P50 / P95 / max ms | n（块调用） |
| --- | --- | --- |
| markdown-block-layout-paint | 0.01 / 0.02 / 0.17 | 2000 |
| virtual-source-font-layout | 0.00 / 0.00 / 0.14 | 2000 |

媒体延迟单独保存在 `final-r7-media/rich.json` 的mediaTimings及HTTP请求时间。文本固定槽没有媒体，因此不把媒体耗时塞入文本parse/layout分布。

## 资源观察

`final-r7-suite/02-resources/resource-summary.json` 包含主PID及process-tree的working set/private bytes。图片50–74次/75–99次的private中位数为234.57/237.50MiB；前12次/后12次文件切换为249.61/250.74MiB，最终观察峰值252.25MiB，未见随100图片持续线性增长的趋势。Working set在原生对话框/切换期间被Windows裁减，不能把该下降冒充应用释放private内存。

URI状态LRU64并调用forget_image；worker快照/结果各单槽、累计编辑256；转换/语义页各有条目与32MiB预算，富媒体1024条/64MiB与4待执行+1执行。诊断预览记录2048、媒体256、滚动决策256。上限通过源码/回归检查，运行时用淘汰后的再次HTTP请求和有限内存窗口交叉验证。该有限窗口不证明无限时长无泄漏，GPU驱动显存也不在private bytes统计里。

## 失败记录与限制

- R2基线detached worktree与候选共享target，造成新main链接旧vendor。字符串和真实图片UI揭示混用；R3起隔离target并保留rustc路径和三sentinel。R2图片结果不纳通过。
- 旧中段漂移、通用jump误认编辑、R5/R6近页首补偿帧错误均保留在对应matrix目录；`final-r6-trace-matrix/rich.json`是帧顺序根因原件。原样矩阵未改成在中段源码输入来绕过问题，另加真正中段滚动条场景。
- 早期录到Codex窗口、Windows换行展开导致fixture跨声明阈值、瞬时按键/对话框等待不足、媒体修复未送达及报告读取竞争的失败数据保留，未合并进40次通过系列。
- 最初R5从Windows看不到容器target卷，首个启动失败目录保留；随后用docker cp取出实际exe，再跑全新目录。没有用该失败启动当GUI结果。
- 首次无快照显示准备说明；解析超过5秒或worker退出显示Retry preview入口。当前普通16MiB实例没有自然触发该故障条件，没有做挂死/终止parser线程的GUI注入，不能宣称该故障恢复已原生GUI验证。慢/坏/超时媒体恢复已真实执行。
- 只验证上述Windows/OpenGL主机，未做Linux桌面、macOS、WARP或各类硬件的本轮同等测试；也未测物理显示器呈现时间、真正线程CPU时间或无限时长内存。
- 最终R7仅1MiB−1做12次性能复测，完整五档40次比较属于R4。最终接口/滚动帧顺序已用冻结二进制完整功能矩阵复核，仍应保留这两者的范围差异。

本地候选可审查，不代表正式发行包已经替换。

## 本地整理与历史二进制归档（2026-10-27）

本地日常入口统一为 `D:\MDcard\App\MDcard.exe`，桌面 `MDcard.lnk` 指向该入口。初次整理时的程序是冻结R7验收之后的格式整理重构建，二进制来源提交 `c980df103c01121cf3c42eb71fdd1c118b650db7`，SHA256 `e898a5528a4fe0fb5f29860fb595dfad998307f75b3c1b521cfd718714db7d0d`；后续更新的具体身份见本地 `App/manifest.json`。完整离线renderer/runtime随App放置，本地包装、启动与清理记录在 `App/evidence/`。这些单独的安装检查不替代本文R4/R7原始性能样本，也没有把旧afbe哈希的数据改名为新exe的全量复测。

代码PR为 [#6](https://github.com/huwany1/MDcard/pull/6)，创建时待合并；GitHub Release资产未替换。App是本地更新入口，不是正式Release新版本。

本文提到的散落历史exe已集中封存至 `D:\MDcard-local-archive\20261027-preview-acceptance-binaries.zip`，ZIP SHA256 `11cbfb96f4f865c78b737815f6486357931303ced49d1a9a8f3dbcbb8abe1e58`；独立索引为同目录 `20261027-preview-acceptance-binaries.index.json`。ZIP中按原仓库相对路径保存baseline、仅计时baseline-measured、实测R4与冻结R7共4个exe，另附每个角色的manifest、R7完整源码manifest、基线计时diff和本次统计JSON。压缩CRC和解压后逐项SHA256均已验证。原始候选路径仅表示测量时路径；清理后需从ZIP中按同名条目取回，不能要求旧散落exe继续留在工作区。此前R4只读副本也由该ZIP替代，其edf3哈希不变。

验收视频、帧、动作与性能原件仍保留在 `evidence/preview/2026-10-27/`，没有删除；用户未追踪的 `docs/ui-ux-proposal-2026-10-27/` 未改动。清理前后文件清单、删除范围、哈希和实际释放空间记录在 `App/evidence/cleanup-before.json`、`cleanup-after.json`。

## PR 审查后的修复

审查指出中间大文档快照没有保留完整源码时，清空正文会将未知源码误认成空字符串，使旧预览被错误地标记为当前版本。现显式区分源码未知与已解析的空正文，补充回归先复现旧问题，再验证等待期间坐标不可用、空正文解析后最终一致。文件列表的合并刷新在上次扫描后等待两秒，现在主动安排到期唤醒；回归检查真实 egui 帧的重绘期限及后续索引内容。上述修复后的构建和测试与此前R4/R7测量分别记录，不改变原始性能数据。
