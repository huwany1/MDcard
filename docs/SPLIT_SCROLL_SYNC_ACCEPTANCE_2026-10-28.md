# Split 双向内容滚动同步验收（R19，2026-10-29）

当前冻结候选为 **R19**，已基于更新后的 main 重新构建并通过完整代码检查；新版 Windows UI 的内容往返、滚动条、Page、跨文件、编辑及撤销重做已有同字节实测。**六尺寸各100次/方向的正式最小报告配置已通过首次可见响应P95及实际driver确认后停止门槛；完整报告控制、双侧60秒录像、重排与16MiB旧快照场景已实测；观测器最终输出失败及设备/平台未验证项分列保留。**

## 候选身份

- [mdcard.exe](D:/MDcard/artifacts/split-scroll-2026-10-27/candidate-r19/mdcard.exe)：74,871,808 字节；SHA256 `6adea12ab9c430a434f85e452c7c3460e062fb51542e10d5c0e541e45b81392a`。
- 分支 `codex/split-scroll-sync`，源码提交 `11806c3c1206d54e01f39111e209441030f5b1bb`；已 rebase 到 main `6df8b07312496ef1af73b4ea8e9be8881d388c62`。此前检查点及 before-rebase.bundle 保留。仅本地提交，未推送、安装或发布。
- source_inputs SHA256 `b8a16e06f61187d324822b6ca8aeac543f4e103daeac9ec78dbee5fc5e35df00`；冻结时实现 patch 为空，SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。
- [MDcard-source.zip](D:/MDcard/artifacts/split-scroll-2026-10-27/candidate-r19/MDcard-source.zip) SHA256 `70488852457613432e2f1582af418ce873bed7ef5fdc66cf3a04f507be236608`。373 项输入包含新 `src/virtual_input.rs`；主代理逐项核对 ZIP/工作树零差异，exe 与 candidate-r19-probe 逐字节相同。因此下列 probe 原件可按同字节身份用于 R19，无需改名。
- 本生成报告明确排除在源码 hash/patch/ZIP 之外，原因记录在 source-manifest.json，最终另计 SHA。用户 `docs/ui-ux-proposal-2026-10-27/` 未纳入候选，也未改动。采集工具包和最终报告分别计算SHA，见交付清单；报告不参与自己的source hash。

独立身份原件：[master-r19-identity-audit.json](D:/MDcard/evidence/split-scroll/2026-10-27/checks/master-r19-identity-audit.json)。

## 运行时行为

普通和虚拟源码编辑路径共用同一文档的内容坐标。锚点来自实际绘制的源码/Markdown Galley；表格使用渲染 AST 真实行的源区间、相邻行边界及行内进度，末行使用实际 extent。无外 pipe、嵌套引用、代码中的 pipe、很高末行及语义分页重复表头有回归；身份依 source range、版本索引与连续 provenance，重复字面文本不能取得身份。

只有正文真正消费的纵向输入才能取得驱动权。程序跟随、阅读恢复、布局补偿、光标/选区自动显露不回传为新输入。单槽 pending capture 等待最新 driver 实际绘制，target/anchor/confirmed sequence 同时绑定当前会话、revision 与 parser epoch；旧快照不使用过期源码坐标。

R19 修复了文本最终落位过早结束：插值预测接近目标时，不能先消耗 settled 资格；须由下一次实际 glyph 布局反馈确认后再完成最新 sequence 的一次亚像素落位。保留静置 deadband；fallback/cache-only 文本没有新增落位权限。300轮独立 literal 检查在30/120fps下通过，仍有非零有界残差。

虚拟 IME 窗口/实际布局原点固定至 Commit 输入处理完；阅读编辑恢复只允许同文档、同 width/font、旧新都唯一且完整文本未改的首个可见块，证书最多64KiB。匹配仍进行时仅暂保显示，精确文本结果优先，只有真实非精确 fallback 才扩展保像素恢复。未放宽跨块及重复块保护。

历史位置64项/侧、表格边界128项、pending单槽均有界；滚动热路径不增加全文解析/复制/全量挂载。

## 九项验收状态

| 项目 | 当前 R19 状态 | 证据与边界 |
| --- | --- | --- |
| 1 范围/隔离 | 通过已测范围 | 同文档普通/虚拟/短空稿及旧会话回归；Windows Write/Read/Split、跨文件拒绝旧命令并重新接管通过。16MiB进程首次加载另有单次实测，未清系统缓存。 |
| 2 双向/最新意图 | 通过已测范围 | 五组各20轮、双侧滚动条、双侧PageDown、横向隔离、40次快速接管；物理触控板/惯性设备未验证。 |
| 3 内容精度 | 通过已测范围 | 唯一 literal/源byte/实际rect独立 oracle；下表实际最大漂移均小于各自一行。300轮30/120fps检查保持原容差；不声称零漂移。 |
| 4 平滑/停止 | 通过已测范围 | 双侧各60s/1200输入、40次接管、3次resize及双栏录像；一行停止门槛通过。App update wall不代表GPU/显示帧时，不承诺每帧60fps；64.84ms长尾保留。 |
| 5 延迟 | 通过已测方法 | App虚拟输入→右屏独立DXGI桌面捕获；不把hook ack/UI字段当首次可见。六档每向100共1200有效，桌面P95均达预算，旧R18超预算原件保留。 |
| 6 编辑/动态重排 | 已测场景通过；媒体后置未完成 | 448bytes编辑历史、16MiB旧快照期间2819bytes编辑/Preview本地滚动/恢复最新owner、实际resize；可见延迟图片解码及heading保位通过。offscreen媒体实际重排未复现，code100后置脚本未执行，不称完整媒体矩阵通过。 |
| 7 资源 | 已测资源对照完成；观测器协议失败 | 同文档2分钟R19与main基线顺序对照：update wall P95约+2.63%/+4.39%、private中位更低；没有长期泄漏证明。两者observer最终exit2/retention裁剪/os5写入失败均保留，外部合并样本完整。 |
| 8 文稿/焦点/选区 | 已测普通/虚拟滚动与编辑通过 | 根单测六尺寸纯滚动逐字全文/revision/dirty相等；Windows对应App输入场景状态保持。最终R19真实OS中文IME/物理键盘未测；历史R17实测另列。 |
| 9 检查/源码交付 | 通过当前代码检查与身份 | fmt/clippy、根debug/release、7包9非空单元、Linux/Windows release通过；373项源码ZIP/工作树匹配。工具包与报告另有独立交付SHA。 |

## Windows 实际内容精度

Windows DPI=1.25。主代理独立读取原始 literal/source byte 与实际 Galley rect，原20轮断言未放宽；每组只初始化一次原marker。表格行高不同，top-row进度与同一栏marker累计漂移分别验证，不强迫所有首个完整行y相等。

| 场景 | 往返轮数 | 最大累计漂移 Source / Preview（逻辑px） | 实测行高 Source / Preview |
| --- | --- | --- | --- |
| code-ordinary | 20 | 0.78125 / 0.78125 | 17.59375 / 15.18750 |
| code-virtual | 20 | 1.59375 / 1.59375 | 17.59375 / 17.59381 |
| table | 20 | 0.81250 / 3.18750 | 17.59375 / 17.59375 |
| table-reduced | 20 | 0.78125 / 1.62500 | 17.59375 / 17.59375 |
| chinese | 20 | 0.00000 / 0.00000 | 17.59375 / 17.59375 |

原件：[master-r19-probe-content-audit.json](D:/MDcard/evidence/split-scroll/2026-10-27/master-r19-probe-content-audit.json)；每组 `root-r19-probe-*` 目录保留 actions、实际几何、PNG、screen.mp4、时间戳及当次harness副本。普通代码marker byte5680；表格byte15559。原记录仍名为probe，exe相同由独立身份审核证明。

另有30/120fps各300轮 Rust 实际布局 literal 检查：最大Source13.625px（行高17.59375）、Preview12px（行高15.1875），第299轮残差0.78125/0；这是该300轮范围内的有界非零结果。旧原型失败100轮、过早settled诊断以及修后结果均保留：[master-post-rebase-300-cycle-audit.json](D:/MDcard/evidence/split-scroll/2026-10-27/checks/master-post-rebase-300-cycle-audit.json)。

## 输入矩阵与编辑恢复

[root-r19-probe-input-matrix](D:/MDcard/evidence/split-scroll/2026-10-27/root-r19-probe-input-matrix/input-matrix-result.json) 的11项为双侧bar drag、双侧PageDown、horizontal-independent、40次rapid-takeover、Write、Read、重新Split、旧会话拒绝、重开文件后新输入。均通过App原始事件路径，不直接设置offset/driver。

[root-r19-probe-edit-anchor](D:/MDcard/evidence/split-scroll/2026-10-27/root-r19-probe-edit-anchor/edit-anchor-result.json) 通过原编辑器插入448 UTF-8 bytes、Undo、Redo、Undo-final。每步等当前mapping及reading pending结束，literal byte `5680→6128→5680→6128→5680`，预览y恒4.5625；最后通过UI保存回原始磁盘字节、dirty=false。保留视频/插入后截图及主代理视觉复核。这是App内原编辑器事件验收，绝非OS IME或物理键盘证明。

## 输入与测量边界

用户保留左屏 `[-2560,0,0,1440]` 工作，MDcard仅右屏 `[0,0,2560,1440]`，非激活窗口 `[40,40,1590,1064]`。脚本不调用全局鼠标/键盘/激活/捕获/剪贴板，不称独占整桌面。per-monitor DPI=2；正式延迟采样（minimal/fullreport）识别有标题真实主窗口，并逐次验证两个pane ROI的10个位置属于该root HWND，避免同PID影窗和遮挡误判。用户共享foreground/cursor可以变化，不将这种变化归为脚本输入。

只有 `MDCARD_VIRTUAL_INPUT=1` 时启动Windows本机stdin入口：16KiB/行、32事件/批、有限队列、reader+唯一writer最多2线程。seq单调且有事件时buffer/generation/revision须精确匹配；无效/EOF/换会话取消held状态。普通启动None，无stdin线程、轮询或输入过滤。事件进入eframe raw_input_hook：Move/Wheel/PrimaryButton/有限导航Key，以及仅fixture使用的<=4KiB Text/Undo/Redo；不直接改正文或滚动状态。显式测试上下文的egui focused=true不改变OS焦点。

时间分开保留：客户端原始写入→hook消费ack→UI实际drawn geometry→GL callback（无glFinish）→DXGI首次真实BGR变化。QPC ticks/frequency可跨进程比较；过程相对elapsed_us不能单独拼到桌面时间轴。GL callback不是GPU完成、Swap完成或物理面板扫描。

当前R19桥接state的dirty字段调用Document::dirty（text与saved逐字比较），因此显式测量模式在每次自然update有额外O(n)读取开销；普通未开启入口时不执行此项。此发现不证明任何具体慢样本的根因，正式采样保持冻结源码，原结果完整保留。

独立DXGI仅选择DesktopCoordinates为右 `[0,0,2560,1440]` 的output，限定右窗ROI；仅哈希BGR，alpha作为非可见通道另记录。None保留为fresh_image=false，既不冒充新图，也不当作负像素观察；缺失/无效/超时/错误原样保留。DXcam0.3.0、NumPy2.5.4、comtypes1.4.17仅装于隔离venv，官方wheel SHA与锁定requirements保留。

最初R19 minimal误选无标题shadow HWND：首批桌面数据无效（4次Source捕获缺失）、随后两次ownership前置失败均保留。修正有标题真实root后owned-v3双向各5次有效且UI/GL/desktop无缺失；当时有并发检查，属于通路probe，不计正式性能：[master-r19-probe-observer-adjudication.json](D:/MDcard/evidence/split-scroll/2026-10-27/master-r19-probe-observer-adjudication.json)。

## 正式性能：最小报告配置

已完成六档各100有效输入/方向，合1200有效、0无效，所有raw输入均保留。默认UI/Reading/Rich/Performance/Scroll报告关闭，保留被动bridge state/paint与独立DXGI；未录像或并发编译/重型tests。用户仍可在左屏工作，不是独占桌面环境。完整报告控制另列，不能混成同一配置样本。

以下ms均为nearest-rank的P50/P95/max；Source/Preview是真实消费该事件的驱动栏，逐次核对新sequence、session、实际两侧位移方向、10点窗口ownership与accepted ack。双向各100，UI/GL/desktop缺失均0。

| 精确UTF-8 bytes | 方向 | 尝试/有效/无效 | 桌面BGR首次变化捕获完成 P50/P95/max | UI实际drawn P50/P95/max | GL callback P50/P95/max | 桌面逐样本超预算 |
| --- | --- | --- | --- | --- | --- | --- |
| 8,192 | 源码→预览 | 100/100/0 | 16.60/24.70/33.40 | 2.91/11.40/12.45 | 3.29/11.76/12.81 | 0 |
| 8,192 | 预览→源码 | 100/100/0 | 16.18/22.88/33.30 | 2.82/12.00/14.57 | 3.56/12.64/15.30 | 0 |
| 65,535 | 源码→预览 | 100/100/0 | 16.17/26.44/29.44 | 3.63/12.69/14.32 | 4.12/13.12/14.77 | 0 |
| 65,535 | 预览→源码 | 100/100/0 | 20.74/27.11/30.68 | 10.99/13.43/18.55 | 11.66/14.18/19.31 | 0 |
| 65,537 | 源码→预览 | 100/100/0 | 16.89/25.34/35.05 | 2.64/12.56/13.82 | 3.05/12.93/14.18 | 0 |
| 65,537 | 预览→源码 | 100/100/0 | 21.35/29.67/32.78 | 11.72/14.15/14.43 | 12.36/14.81/15.08 | 0 |
| 1,048,575 | 源码→预览 | 100/100/0 | 20.37/27.71/31.53 | 9.79/12.64/14.47 | 10.23/13.07/14.88 | 0 |
| 1,048,575 | 预览→源码 | 100/100/0 | 22.37/28.91/35.06 | 11.55/14.24/20.53 | 12.19/14.84/21.22 | 0 |
| 1,048,577 | 源码→预览 | 100/100/0 | 19.98/26.63/29.32 | 9.67/12.87/16.33 | 10.19/13.32/16.76 | 0（额外50ms比较） |
| 1,048,577 | 预览→源码 | 100/100/0 | 22.72/30.39/45.48 | 11.94/14.35/14.76 | 12.63/14.97/15.45 | 0（额外50ms比较） |
| 16,777,216 | 源码→预览 | 100/100/0 | 23.04/31.53/37.31 | 11.34/15.88/17.09 | 11.77/16.38/17.63 | 0 |
| 16,777,216 | 预览→源码 | 100/100/0 | 24.63/32.44/46.14 | 13.26/16.71/36.90 | 13.99/17.51/37.26 | 0 |

≤1MiB要求P95≤50ms，16MiB要求P95≤100ms，本轮均通过。1MiB+1超出“≤1MiB”字面范围，额外50ms比较单列。上述0次超线来自本轮逐条样本核对，不是由P95反推，也不代表其它机器/硬件输入/未观测样本的保证。hook、UI、GL与桌面各自计时；桌面捕获完成是观察上界，不是物理面板扫描。

### 停止收敛

一行容差只取同current session/revision/epoch、latest confirmed owner/sequence、anchor/target sequence且pending已结束的actual-drawn目标行高，不采用默认19.5px或旧target。下表ms为P50/P95/max，严格≤1px是附加指标。

| bytes | 方向 | 客户端写入→一行 | hook交付→一行 | driver实际绘制确认→一行 | driver确认→严格1px P95/max |
| --- | --- | --- | --- | --- | --- |
| 8,192 | 源码→预览 | 14.49/23.60/26.29 | 14.17/21.60/22.54 | 9.06/18.20/20.19 | 69.40/73.29 |
| 8,192 | 预览→源码 | 21.45/26.45/28.33 | 21.15/24.79/26.45 | 16.57/20.85/21.08 | 69.67/75.25 |
| 65,535 | 源码→预览 | 16.54/24.36/26.66 | 14.88/22.05/24.04 | 9.20/18.97/20.83 | 68.73/74.47 |
| 65,535 | 预览→源码 | 24.82/27.22/28.85 | 21.81/22.62/25.67 | 13.91/19.61/20.89 | 71.13/75.31 |
| 65,537 | 源码→预览 | 15.09/24.73/33.35 | 14.31/21.51/28.39 | 8.91/17.47/20.79 | 69.40/71.84 |
| 65,537 | 预览→源码 | 26.64/33.77/34.95 | 21.57/29.05/29.76 | 13.98/26.77/27.76 | 76.28/83.20 |
| 1,048,575 | 源码→预览 | 23.75/27.87/32.74 | 21.60/24.48/28.65 | 13.76/20.69/20.90 | 69.50/72.69 |
| 1,048,575 | 预览→源码 | 18.33/26.27/35.59 | 14.70/21.70/35.46 | 6.95/13.99/27.73 | 62.47/76.30 |
| 1,048,577 | 源码→预览 | 23.60/28.30/30.73 | 21.59/25.32/28.56 | 13.80/20.77/21.44 | 69.55/73.72 |
| 1,048,577 | 预览→源码 | 19.70/27.68/35.98 | 14.78/21.93/35.65 | 6.97/14.08/33.39 | 69.38/81.98 |
| 16,777,216 | 源码→预览 | 26.04/32.58/36.43 | 23.35/30.14/31.18 | 14.00/20.89/23.04 | 72.98/76.02 |
| 16,777,216 | 预览→源码 | 20.69/28.55/49.82 | 16.61/23.71/43.71 | 7.10/14.36/34.43 | 65.24/82.97 |

按“驱动栏已停止且布局当前”可验证起点，本轮confirmed-driver-drawn→一行最大34.43ms，≤1MiB的150ms、16MiB的250ms门槛均通过；raw-input口径也逐条保留。最小配置没有独立仪表化ScrollArea最后消费native事件的准确QPC，因此hook→一行仅表示App raw事件交付，不冒称body last-native；该精确起点列为未单独测量。没有把首pass旧target等于旧offset记作0ms通过。

### 观察器与静置

| bytes | fresh图 / None轮询 | error/recovery/frame drop/poll drop | UI静置实际时段/状态数 | Source/Preview offset变化px |
| --- | --- | --- | --- | --- |
| 8,192 | 4165 / 4047 | 0/0/0/0 | 2.0001s / 21 | 0.0/0.0 |
| 65,535 | 4454 / 3768 | 0/0/0/0 | 2.0001s / 21 | 0.0/0.0 |
| 65,537 | 4516 / 3716 | 0/0/0/0 | 2.0002s / 21 | 0.0/0.0 |
| 1,048,575 | 5030 / 3225 | 0/0/0/0 | 2.0001s / 21 | 0.0/0.0 |
| 1,048,577 | 5073 / 3168 | 0/0/0/0 | 1.9930s / 21 | 0.0/0.0 |
| 16,777,216 | 4980 / 3267 | 0/0/0/0 | 2.0000s / 21 | 0.0/0.0 |

各档静置尾部512KiB原始transport均包含两端与中间21个UI状态，实际offset范围各自恒定，未只比较两个端点。帧号仍可能因系统/光标/用户活动推进，不主张完全无重绘；静置offset不被重复改写。fresh与None分别保留，None不是新像素也不是负观察，alpha为255且仅BGR参与hash；观察器错误、恢复、frame/poll drop均0。

独立复算原件：[implementation-r19-minimal-independent-audit.json](D:/MDcard/evidence/split-scroll/2026-10-27/checks/implementation-r19-minimal-independent-audit.json)。它从全部actions、per-sample时序及小型observer记录重算nearest-rank，1200原输入前置全部通过、0统计差异；每档目录含independent-statistics/independent-samples、原始capture鲜帧/None记录、QPC、ownership、stderr与harness副本。[运行命令与时间索引](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-minimal-runs.json) 固定本轮入口。旧失败与所有缺失仍单独保留。

## 完整报告配置控制

同R19/App入口/右屏DXGI，1MiB−1及16MiB各100/方向，共400有效、0无效、UI/GL/desktop缺失均0。以下ms，桌面列为P50/P95/max，两个UI/GL值仅P95。

| bytes | 方向 | 桌面P50/P95/max | UI / GL P95 | 逐样本超明文预算 | driver确认→一行max |
| --- | --- | --- | --- | --- | --- |
| 1,048,575 | 源码→预览 | 34.46/46.91/198.48 | 31.85/32.47 | 3 | 24.28 |
| 1,048,575 | 预览→源码 | 36.83/47.94/137.14 | 32.92/33.55 | 4 | 22.44 |
| 16,777,216 | 源码→预览 | 41.78/53.27/111.75 | 36.78/37.55 | 4 | 61.23 |
| 16,777,216 | 预览→源码 | 37.58/45.79/137.02 | 31.47/32.17 | 1 | 33.90 |

两档P95均通过各自50/100ms预算，但保留12个单样本超线：1MiB−1为3/4个、16MiB为4/1个，不能称全报告每次输入都预算内。它与最小报告矩阵是相同二进制的不同观测条件；非随机顺序、共享桌面/调度等均可能参与，不能把差异完全归因报告I/O。[主代理独立统计](D:/MDcard/evidence/split-scroll/2026-10-27/master-r19-full-control-overview.json) 及 `control-r19-full-*` 原始样本完整保留，不替换1200最小配置样本。

### 实际采集间隔

| bytes | 全部fresh+None轮询间隔P50/P95/max ms | 仅fresh图间隔P50/P95/max ms |
| --- | --- | --- |
| 8,192 | 8.50/11.97/28.36 | 8.52/68.33/339.69 |
| 65,535 | 8.50/11.95/23.14 | 8.50/59.20/382.84 |
| 65,537 | 8.49/11.92/27.59 | 8.49/53.01/340.09 |
| 1,048,575 | 8.49/11.92/23.09 | 8.48/25.81/305.84 |
| 1,048,577 | 8.50/11.92/23.56 | 8.51/25.73/298.34 |
| 16,777,216 | 8.50/11.99/35.77 | 8.52/25.75/331.31 |

fresh图长间隔包含静置期间没有新desktop图的情况，不能将其当作每次输入都晚响应；None仍不当负像素观察。[独立fresh BGR重放](D:/MDcard/evidence/split-scroll/2026-10-27/checks/implementation-r19-desktop-replay-audit.json) 从输入前baseline、LastPresent QPC和实际hash重新找首次变化，1200条与per-sample严格0差异。

## 双侧60秒录像与资源基线

R19及隔离main基线采用相同1,048,577bytes文档、窗口1550×1024、DPI1.25、font15、动效开启；每侧60秒、各1200个App输入，随后40次接管和3次resize。仅UI_REPORT及FULL_UPDATE_REPORT，其他巨型报告关闭，psutil每0.5s记录进程private/RSS/CPU；同10fps右屏整窗录像。不同模型的滚动行为保持原样，未为基线强制对齐相同像素；先后顺序非随机，用户可在左屏工作，结果不是同步功能CPU成本的严格因果估计。

main基线取自 `6df8b07312496ef1af73b4ea8e9be8881d388c62`，仅修改main以接入相同测试transport/被动state/paint，新增相同 `virtual_input.rs`，未移植SplitScroll、未改原有滚动语义。基线exe SHA256 `90675ffaaac1390951014c3e961aba9aa63749ccfc04ce13dc708254a5cee0c9`；git archive、transport-only.patch及369项source manifest保留。基线existing_* offset只表示原有字段，不冒充新增actual-drawn仪表。

| 驱动栏 | update wall P95 R19/基线ms | P95绝对/相对变化 | private中位R19/基线MiB | RSS中位R19/基线MiB | CPU中位R19/基线（单核100计） |
| --- | --- | --- | --- | --- | --- |
| Source | 1.5051/1.4666 | +0.0385ms / +2.63% | 131.72/133.18 | 158.84/159.64 | 96.6/103.3 |
| Preview | 1.4821/1.4198 | +0.0623ms / +4.39% | 136.88/137.98 | 164.02/164.86 | 36.5/33.4 |

两个phase的App update wall P95均低于16.7ms、比基线增幅小于10%；private中位未超过基线。CPU中位Source为−6.7个单核百分点（相对−6.49%），Preview为+3.1点（相对+9.28%），不把这些非随机波动全归因同步；CPU包含本进程观察线程工作。R19实际max为Source7.98/Preview64.84ms，基线22.11/130.79ms，长尾保留，不宣称每帧都60fps或CPU wall就是物理帧时间。两分钟数据不构成长期无泄漏证明。

**观测器最终协议失败单列：**两者通过窗口定向WM_CLOSE结束后均exit2，FULL_UPDATE final flush_complete=false，分别retention裁剪9873/9764项（cap8192），publication_failures=1且Win os5。不能说正常exit0或删去裁剪项。外部每0.5s按(frame,pass)合并18065/17956条，分别严格等于submitted=processed，queue dropped=0，主代理逐条复算无缺样/重复/统计差异。因此帧统计来自完整外部合并集，但最终raw observer报告本身不完整。另见下节未开启此observer的R19正常exit0对照。

[R19 132秒双栏录像](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-resources-motion/screen.mp4)、[基线录像](D:/MDcard/evidence/split-scroll/2026-10-27/baseline-main-6df8-r19-resources-motion/screen.mp4)；两目录的first/last PNG从各自视频帧抽出，video-frames.json有实际捕获时点，10fps不能冒称60fps或50ms捕获精度。[资源对照](D:/MDcard/evidence/split-scroll/2026-10-27/checks/r19-resource-baseline-comparison.json)、[主代理样本审计](D:/MDcard/evidence/split-scroll/2026-10-27/checks/master-r19-resource-sample-audit.json)、[基线身份审计](D:/MDcard/evidence/split-scroll/2026-10-27/checks/master-r19-baseline-identity-audit.json)。

## 初始加载与旧快照另设场景

16MiB单次**进程首次加载**（未清OS文件/字体/GPU缓存），客户端启动QPC至首UI state263.38ms、首GL callback265.88ms、首mapping current1460.70ms；有标题窗口可见粗观察约299.72ms（20ms轮询及循环调度）。这些不是清缓存冷磁盘启动，也不是首像素/物理显示耗时，不混入stable滚动1200样本。

随后源码页首输入2819bytes并在新revision时立即向Preview发40point滚动。实际记录50个current=false状态；trace frame213 Preview user=true，seq2，native2359.28125→2399.28125，旧快照可读且显示Updating preview。此时anchor=null、confirmed=null、target_sequence=null、Source target=null；旧Preview numeric诊断target仍残留但没有对应seq、不作为跟随应用，不能凭残留数值冒称回传。Source自身编辑/布局恢复可移动，但不由旧源码byte驱动。

恢复后driver仍Preview seq2，当前revision1，新实际glyph byte6588；选区[2819,2819]和focus Some(7EAE)保持。恢复后的stale-input lineage没有伪造新布局native confirmed/anchor sequence，不能套stable测量口径计0ms；最新owner保留，新锚点取自current实际布局。Undo回原稿、磁盘字节未改，未开启FULL_UPDATE_REPORT，经WM_CLOSE正常exit0。[原始状态/结果](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-startup-pending/startup-pending-result.json)、[旧快照PNG](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-startup-pending/pending.png)、[恢复PNG](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-startup-pending/restored.png)、[全过程10fps录像](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-startup-pending/screen.mp4)。主代理也独立查看了旧快照图和状态。

## 受控延迟媒体及失败边界

localhost HTTP server实际返回PNG，v4可见图像decode/layout后Preview extent `5779.4375→6519.4375`（+740px）；同heading literal byte2的Source y−2.03125不变，Preview−2.4375→−1.625（0.8125px），均小于实际各自行高；driver/sequence、正文、焦点选区保持，9项核心检查全部true，主代理实看实际蓝图PNG。

**整个v4脚本未通过：**核心检查之后追加gotoTop/code100检查时，harness误发8000point，超过入口4096上限，得到正确invalid_event拒绝并AssertionError；raw passed=null保留。code100后置未执行，不能称媒体完整矩阵通过。v1缺GEOMETRY使visible_runs=0，v2 Windows CRLF转换导致预期byte与实际byte不一致，均属于无效测量前置；v3 HTTP虽返回但offscreen节点没有观测到extent重排，20s timeout保留，不能据此说产品已失败或已通过该布局定位。候选源码未因这些harness问题改动。

[核心9项与原始脚本失败裁定](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-delayed-media-v4/media-adjudication.json)、[实际蓝图及heading](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-delayed-media-v4/after-completion.png)、[HTTP时序](D:/MDcard/evidence/split-scroll/2026-10-27/final-r19-delayed-media-v4/http-events.json)。全部v1–v4原件、各次harness副本及日志保留；GUI测试已结束，不再追加场景。

## 完整代码检查

`RUST_TEST_THREADS=2` 下完整执行 `python3 scripts/check.py --desktop --acceptance`，exit0。fmt、Clippy所有target的-Dwarnings、根debug348pass/6ignored，根release另348pass/6ignored；7个Cargo包9个非空测试单元分别145/120/35/23/34/106/111/24/125，合723pass，另8ignored。两个恢复子进程helper的独立1pass输出不重复计数，空doc-tests不计新单元。

Linux与Windows release新构建通过。新源码完整检查未继承旧包结果。Windows及Linux debug首编译各遇旧egui_commonmark schema缓存，失败原件保留；仅清对应目标该包缓存后重编译通过，未改依赖或锁文件。所有source已在检查后本地提交，提交不改已测源码字节；最终exe与probe SHA完全相同。

原件：[final-r19-check-summary.json](D:/MDcard/evidence/split-scroll/2026-10-27/checks/final-r19-check-summary.json)、[完整desktop/acceptance日志](D:/MDcard/evidence/split-scroll/2026-10-27/checks/final-r19-desktop-acceptance-cache-rebuilt.log)、[根release与先前缓存失败](D:/MDcard/evidence/split-scroll/2026-10-27/checks/final-r19-complete-checks.log)、[Windows build](D:/MDcard/evidence/split-scroll/2026-10-27/checks/final-r19-windows-build.log)。vendored表格Rust路径参与本轮相关回归；renderer JavaScript/运行时未修改，未称历史Node/浏览器测试本轮重跑。Linux只有构建/测试证据，macOS原生未验证。

## 历史失败与未验证项

旧R18两套GDI/DXGI各1200有效输入曾有桌面P95超预算方向，**失败数据继续保留，不改名成R19通过**。R18 report-only控制与WM_MOUSEMOVE无效前置不得证明报告关闭有产品缺陷；PostMessage受共享物理cursor/WM_MOUSELEAVE影响，不是独立输入seat。当前App入口修正测量隔离方式，并未把旧失败删去。

R15虚拟IME曾在跟随后提交raw `ni'hao`，R16/R17真OS微软拼音修复实测已保留；本次R19没有用App Text冒充真IME。R15编辑上方跳heading、R14表格累计超线，以及R19修前文本100轮漂移18.375>17.59375均有原始失败。旧读锚恢复/表格修复与新文本落位分别有实际回归，未改一行阈值。

R18完整报告及两套全部P50/P95/max/缺失/观察区间/一行统计独立保存在[历史报告](D:/MDcard/evidence/split-scroll/2026-10-27/historical-reports/acceptance-r18-before-r19.md)。它属于旧main0cf20/R18，不能外推成当前main6df8/R19重验。

仍未验证或未全部重测：最终R19 OS中文IME/物理键盘、触控板/惯性设备、物理面板扫描延迟、清系统缓存冷启动/macOS原生、offscreen媒体实际重排/code100媒体后置与长期资源场景。本报告逐项保留限制，不能以源码阅读或历史候选替代本候选证据。


## 交付状态

实现源码与候选目录已冻结；本报告区分实现完成、上述实际验收范围和未验证条件，不能外推所有设备/平台都通过。既有本地已安装App、快捷方式没有被替换；运行新版本需打开candidate-r19/mdcard.exe。没有push、安装或正式发布。最终工具包、独立报告SHA与各artifact身份列在candidate-r19/delivery-manifest.json；生成报告不参与自己的源码hash。
