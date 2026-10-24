# MDcard 审计后续：并发搜索与导出取消（2026-10-22）

状态：本轮三项修复均已通过主代理独立验收，最终 R2 双平台完整候选包、实际 GUI、负载性能和九格式产物检查通过。基线 `90c1d4b`，工作分支 `codex/bug-audit-2026-10-22`，开始时工作区干净。沿用 1 名子代理调查实现、1 名主代理独立验收。关联原台账 RK-004 / RK-006；本文件不替换上一轮六项已修复问题的历史证据。

## 范围和判定

- 搜索：保留原并发 53.290059ms 失败样本，区分查询排队、worker 处理、引擎墙钟、UI 轮询/绘制及 OS 调度。寻找并消除可证明的重复工作；实际测量跨过 10 秒全量字节复检周期，并验证同大小/mtime 的内容修改确实发布。不得把有限语料测量改写成任意 CPU 负载下的硬实时保证。
- 取消：复现返回瞬间的活进程与退出后的僵尸差异，验证派生/独立进程组、退出重收、无关进程、旧导出目标、临时目录、重试和正常关闭。检查 Rust `kill_tree` 与 JS `killOwnedTree` 两条路径，不能只增加 sleep、放宽阈值或忽略失败。
- 原始发行和上一轮候选不覆盖；新源码、候选与证据由主代理另行冻结。

环境为 Windows 工作区 `D:\MDcard`，Docker `mdcard-dev` `/work`，Linux 内核 `6.18.54.1-microsoft-standard-WSL2`，Rust 1.92。容器可见 20 CPU；实现侧性能对照固定 CPU 0–3，用 0 或 4 个持续计算线程作可重复资源竞争。禁止在同一对照期间并行编译；主代理实际 GUI 与包检查独立进行。

## 问题、证据与修复

### FU-001：RK-004 并发搜索尾延迟

原失败位于 `evidence/bug-audit/master-review/search-final-concurrent-failed.log`：1600 文档/18,659,070 字节，160 轮查询；最慢墙钟 53.290059ms，worker roundtrip 53.24247ms，引擎墙钟 50.198843ms。该轮约 5.89 秒，不能证明跨过 10 秒全量复检。隔离 release 重跑通过的历史结果也保留，不抹去并发失败。

源码确认的重复工作：每次查询对每个命中文档重新建立全篇换行索引、重新计算全篇内容签名，即使缓存文本未变、匹配在开头也进行两次额外全篇遍历。这是可消除的重复工作；墙钟仍受 CPU 调度及文件复检竞争影响，不能将超时完全归因调度。

新增实现侧 ignored 专项 `followup_search_timing_across_full_verification_and_cpu_contention`：持续 14 秒、1600 文档、四类查询；保持文件大小和 mtime 修改 `needle → needlX`，不主动请求刷新，断言周期完整字节复检能发现。每轮记录发送到接收墙钟、worker roundtrip、引擎墙钟与 Linux `/proc/<线程>/schedstat` 的 worker 实际运行时间增量；最后一个指标仅用于 Linux 诊断，不当作 UI 或物理显示时间。

证据目录 `evidence/bug-audit-followup/implementation/`：`search-before-idle.json/.log`、`search-before-load.json/.log`，原始样本和构建日志均保留。修复前/后结果见实现侧确认与验证，完整逐轮JSON保留。

### FU-002：RK-006 导出取消偶发活态断言

原始失败位于 `evidence/bug-audit/implementation/core-cancellation-failed.log`；历史发行 `docs/releases/0.1.1.json` 也记录同类并行瞬态。基线代码先 STOP 根进程，最多四轮通过 `/proc` 发现/STOP 后代，发 KILL 后立即返回；既未确认所有 STOP 完成，也未等待所有已跟踪后代进入退出状态。尚不能仅从这些源码事实断定原失败是观测竞态还是实际漏杀。

新增 ignored 真进程压力探针 `followup_cancellation_handoff_records_live_vs_terminal_descendants`：120 轮、四计算线程、真实 `setsid sleep`，记录取消返回瞬间状态、随后终态、等待状态、无关进程存活；测试进程临时作为 subreaper，只回收已知夹具后代，避免把僵尸累积交给容器 PID 1。该探针保留“返回时不得活着”的断言，不靠延后读取让原判据静默通过。

Rust / JS 的根因、修复和故障回归见下文。

## 完成门槛与限制

本轮 before/after、针对性回归、独立核心、严格检查、双平台构建、完整候选包、实际 GUI/导出旧目标保护及产物散列均已验收。完成仅覆盖本轮冻结范围；下文保留的历史、中间失败和未验证边界不被抹除。

## 实现侧确认与验证

### FU-001（性能，P2，关联 RK-004）

触发：1600 文档反复全局查询，索引快照不变，或跨10秒全字节复检。预期：查询复用不可变索引派生数据，字节变化仍失效；实际：每次查询都重算全文 stamp、分配全篇行号数组，全复检还无条件重建未变化文档的 metadata/stamp/Arc。代码直接证据见 `src/search.rs` 查询和 refresh 路径。

修复：在 Indexed 中缓存文本签名；匹配位置按序只扫描到当前命中的换行，不重复分配完整行号表；全复检仍打开文件比较原始字节，仅在字节、len、mtime全部一致时复用原 Arc。dirty overlay 每次计算自身签名，未使用磁盘缓存冒充当前稿件。自动回归 `full_verification_reuses_identical_snapshot_but_invalidates_same_stat_edits` 在修复前失败（`search-full-reuse-before.log`），修复后同时验证 Arc 复用与同大小/mtime字节改动失效（`search-full-reuse-after.log`）。主代理的独立 Unicode行号、EOF、dirty overlay及缓存回归另行验收。

计时边界：Response 新增 queue_ms（发送到worker开始）、processing_ms（worker开始到响应完成）、completed_at（用于poll_ms接收延迟）；保留原engine elapsed和roundtrip。GUI report另输出full_verifications与refresh_ms；该计数仅说明后台全扫描执行，不自动证明所有文件无错误。专项还断言实际 same-size/mtime 修改被查出，避免仅以elapsed>10秒冒充复检。

初次优化对照（同CPU0–3，1600文件18,613,780字节，每轮14秒，无并行编译）：idle before453样本 P95 14.764ms/max24.635，after569样本 P95 5.371/max6.914；worker实际CPU中位12.106→4.216ms。4计算线程竞争 before310样本 P95 39.263/max47.598，after467样本 P95 16.095/max67.977；CPU中位13.384→4.768ms。after尾部67.977ms仍失败于50ms门槛，最慢worker处理61.200ms/CPU32.428ms/poll6.683ms，不能把它完全归因排队。其在11.56秒附近，不据此断言由复检直接造成。随后补充的 unchanged Arc复用有独立失败/通过证据；最终对照见下表。所有原样本保留，不用均值或较好重跑抹去超时。

### FU-002（取消终态交付，P2，关联 RK-006）

触发：取消有setsid派生进程的真实导出树；预期取消完成前所有已捕获worker均终止，已退出direct child被wait收回，失败准确可见。实际：旧Rust/JS发SIGKILL即返回。Rust120轮压力56次返回瞬间仍活、2秒后0活；已捕获R状态及待决KILL/STOP，随后约毫秒级转Z。旧JS30轮出现1次返回仍活。日志分别为 `cancel-before.*`、`js-cancel-before.*`。因此原单测是信号发出与内核退出之间的终态观测竞态，同时生产代码确实过早交付了取消完成；本次未证明持续2秒残留。

修复：Linux新process_tree使用稳定pidfd发送信号，STOP后确认静止，遍历冻结所有已发现后代直到固定点，再KILL并等待pidfd终态。任务启动前检查pidfd能力，缺失时安全拒绝导出。spawn后所有权捕获失败也kill/wait direct child并报告后代清理不确定。JS两条转换调用统一runOwned：pre-abort不启动，取消/timeout等待树终态与direct-child close；清理失败/流不关闭有截止和cleanup错误，不悬挂reader.join。Windows任务检查taskkill返回码；JS允许已自然退出root的幂等边界。Rust在确认双流关闭后复核已收到的complete，允许自然完成与取消的有限竞态，不宣称任意Windows后代关系均能重建。GUI六语言清理不确定文案不再声称旧输出必保留。私有publication marker仅用于准确说明rename与完成ACK竞态，不另计未经复现的新Bug。

回归：修复后Rust同120轮0次返回仍活，已知后代均由subreaper夹具waitpid收回，无关进程保留（`cancel-after.*`）；Node五项默认真实进程回归全通过（`js-lifecycle-after.log`）。Rust四项默认Linux回归覆盖多层/独立session、稳定终态与收回、注入不可确认终态时明确超时、已reap root幂等，以及下述FU-003。故障注入测试不声称复现了不可中断D态。`cancel-old-binary-replay.*`是误选旧测试二进制的重放，已明确命名，不能当作after。

限制：Linux需要pidfd内核/API和权限；JS仅做starttime身份核对，信号本身仍为PID接口，不声称具有Rust pidfd同等原子防复用能力。终态Z意味着已停止执行；Node本身不能waitpid非直接子进程，不声称消除所有容器PID1僵尸。实际进程与包验收由主代理补充。

### FU-003（根退出而后代仍持管道，P2，新确认）

触发：转换器shell启动后台sleep，根exit0，后代继承stdout；预期不得在后代仍运行且输出管道未关闭时宣称完整转换生命周期成功。直接复现从90c1d4b提取原private run与killOwnedTree、不改算法：8.848ms即返回root-finished成功，后台sleep仍S态。证据 `root-exit-before.json` 与baseline-run/baseline-export-process源文件；夹具随后手动KILL，不遗留30秒后台工作。Rust旧路径try_wait发现根结束后无界join继承管道，也具有直接同类根因。

修复：runOwned以close而非exit判定输出结束；在生成时捕获拥有的session/group身份，根已reap仍能终止同组/会话后代，超时等待close有上限并准确报告。Rust也在spawn后捕获OwnedGroup，根已退出时清理仍持流后代，检查流关闭上限。默认Node回归以100msdeadline验证返回错误、后代终态与<2秒完成；Rust默认回归由独立Python subreaper监督，捕获后明确先wait根，再取消同组sleep、确认waitpid与管道EOF。

限制：开始捕获前已经完全setsid且reparent的未知孤儿无法通过消失的父链、旧group/session找回；本轮不扩展cgroup/作业沙箱，不宣称任意恶意daemon树都可发现。对未确认清理返回准确错误，绝不宣称树完整收净。

### 最终实现侧检查与交接

默认核心、fmt、严格Clippy、Node生命周期测试日志位于implementation目录。最终核心结果和第二阶段性能对照见下表；主代理独立双平台/GUI/包验收未完成前，实现验证不代表全部验收通过。


源码冻结后的实现对照（最终日志 `search-after-reuse-{idle,load}.json/.log`）：

| 受控CPU窗口 | before样本/P95/max ms | final样本/P95/max ms | workerCPU中位 before→final ms | 10秒字节复检 |
| --- | --- | --- | --- | --- |
| CPU0–3，无计算负载 | 453 / 14.764 / 24.635 | 566 / 5.419 / 6.695 | 12.106→4.316 | 两侧实际发现1599 needle匹配 |
| CPU0–3，4持续计算线程 | 310 / 39.263 / 47.598 | 499 / 12.665 / 16.221 | 13.384→4.304 | 两侧实际发现1599 needle匹配 |

最终负载窗口queue中位0.008ms/P95 2.799/max4.064，worker处理中位7.688/P95 12.437/max15.978，poll中位0.134/P95 0.219/max6.598；分项独立分位数不可相加。初始化索引final idle80.798ms/load147.864ms。最终复用优化对照较快，但单次非随机实验不能证明它独自导致所有尾延迟变化。既有53.29ms和中间67.98ms都保留，最终样本通过不等于任意负载50ms保证。

最终默认核心 **101通过、2 ignored**（ignored为两项显式压力/性能专项，分别已有单独运行证据），`core-final.log`；`cargo clippy --locked --all-targets -- -D warnings`通过（`clippy.log`），fmt检查通过、`git diff --check`无错误；Node **5/5**通过。Linux process_tree四项默认测试包含所有权捕获后根已reap的终态、重收和管道EOF。Node专项是Linux源码级生命周期验证；主代理另已提供Windows/Linux portable真实Node回归，最终GUI/全包验收仍以主代理证据为准。

实现改动：`src/search.rs`、`src/search_ui.rs`；`src/process_tree.rs`、`src/export_ui.rs`、`src/main.rs`；`renderer/export-process.mjs`、`renderer/export-process.test.mjs`、`renderer/export.mjs`；`Cargo.toml`与锁文件仅增加已锁定libc直接依赖，六语言ui.json增加清理不确定提示。独立验收脚本由主代理管理。实现交接时未提交，随后产品提交a417583；未覆盖原dist。2026-10-22实现侧源码已冻结交接，未独立验收部分不得据本表宣称关闭。

## 主代理独立核心验收（a417583 中间候选）

- Linux 根核心 101 通过、2 个专项另行运行；Windows release 测试程序本机实际运行并包含原 ignored，96/96；独立 search release 含原 ignored 共55/55。新缓存、中文/CRLF行号、EOF、dirty overlay与same-size/mtime修改回归通过。各套件导入了重复模块，不相加为独立场景数。
- 主代理独立120轮取消压力在返回时0活后代，保留旧before原始56/120。完整fmt与严格Clippy通过。Windows/Linux最终release构建成功。
- 主代理portable Node回归已在Windows/Linux实际运行：正常完成、退出错误、缺失程序、pre-abort、timeout、三轮根/子进程取消立即终态、旧目标/无关进程保护和重试，见master-review/js-{windows,linux}/result.json；最终包还需重验。
- 二进制SHA256：Windows `5b429d75313e1e49cf243eb3d2bdf120ea59644c817854aa7a1573e07dcd94d0`；Linux `427a3e6301b817c7e77dfbb027945b07474295acfbc86387605344ac22bd406c`。主验收日志均位于 `evidence/bug-audit-followup/master-review/`。
- 验收工具校准失败单列保留：Windows需枚举真实owned Save As模态窗口、明确前台与控件位置；过期PPID需用创建时间排除PID复用；GTK输入需选择完整文件名并使用剪贴板，运行目录使用UUID避免旧临时目录混淆。这些无效前提样本未计入产品Bug或通过证据；有效基线GUI见export-baseline-valid与export-windows-baseline-confirmed。

a417583为中间候选，九格式Windows导出已通过，但独立复核发现成功取消分支仍可能无界join，故本候选不作为最终验收交付。

## 主代理退回修正 R2：所有 join 都需已确认双流关闭

独立只读复核发现：a417583只在清理失败和自然退出分支检查管道关闭，成功取消后仍直接join；完全reparent后代持管道时会阻塞。该遗漏属于FU-003既有契约，未重复增加Bug计数。R2新增confirm_export_streams，所有最终join前均确认双reader已结束，1秒截止未关闭报告cleanup错误并有界结束任务；双流结束后复核complete，不能靠complete位越过未关闭流。

四项新增默认回归覆盖真实subreaper已收根、reparented cat持stdout的路径、complete仍不能跳过未闭流、晚到complete和无complete时不吞清理错误。夹具用控制管道释放并wait收回，不增加测试sleep来隐藏失败。主代理R2 Linux105通过+2显式专项，strict Clippy/fmt通过；双平台release已重建，Windows本机测试及最终R2包继续验收。

R2二进制SHA256：Windows `57498cd19c31bfe0f50294a5d053e565e9836a227056dec6c423f5f6d03f891f`；Linux `f5a91aca0b2cffc5faad07fca764a89bc740e55655fa4ffde0125d1f21365fae`。R2完整候选使用新命名，不覆盖a417583归档或正式dist。

Windows a417583实际GUI前三次取消与重试通过，native-close一次CIM存在对象触发失败；当时没有内核终态证据，不能判定为持续活残留。验收工具改为取消前持有稳定进程句柄，完成后立即读取HasExited（不延迟读取）；复核三取消/一原生关闭共42个进程均已退出，无关进程、旧目标、稿件、临时目录和重试通过，证据export-windows-stable-handle。原CIM失败样本保留且未伪称已证明根因；最终R2包需再次实测。

## 主代理最终验收与交付（R2）

产品源码提交 `5d5f2900b4cfbee00340e086bc2afb03d55de9a9`（前序根因修复 `a417583`）；最终证据提交只更新文档和验收工具，src/Cargo/locales/renderer与此提交相同。原基线90c1d4b和既有用户工作得到保留。本轮无范围内未处理的已确认可修复Bug；正常运行环境以外的边界列于下文。

| 项目 | 当前证据与结论 | 状态 |
| --- | --- | --- |
| FU-001 搜索重复工作/缓存失效 | 两阶段before/after、同大小/mtime变字节、中文/CRLF/EOF/dirty overlay、独立55项及最终双平台GUI | 已修复、通过 |
| FU-002 取消终态 | 修复前Rust56/120和JS1/30过早返回；主R2 Rust120次返回时0活后代，真实GUI按终态完成，无关进程/旧目标/稿件/重试通过 | 已修复、通过 |
| FU-003 继承管道与根已退出 | 原版8.848ms过早成功；默认真实进程与四项R2流截止回归、主实际native-close，所有最终join先经过双流终态检查 | 已修复、通过 |
| 独立核心与检查 | Linux串行105通过+2专项显式执行，另一次并行105通过；Windows release测试程序本机含ignored共99/99；search55/55；fmt/严格Clippy/diff检查 | 通过 |
| 包与产物 | Windows22028/Linux21972个归档成员逐个hash、路径/类型、Linux执行权限通过；两个完整包清空PATH各实际九格式、1ms失败保留旧目标，容器/PDF/图片解码与嵌入资源检查通过 | 通过 |
| 最终GUI与实际引擎 | Windows11 26100/125%/NVIDIA OpenGL；Linux Debian12/X11/Xvfb/Openbox/Mesa，从已验证tar实际解压运行；各三取消+一次原生关闭+取消后重试，源字节、旧目标和临时目录断言通过 | 通过 |

最终原生GUI性能：相同1600文件、13,674,346字节、100轮五类query，两个持续SHA256负载进程，包含增删改及周期复检；性能阶段无并行编译/打包/导出。每轮均核对预期命中数量。

| 实际运行环境 | 持续秒/完整复检计数 | UI P95/max ms | worker P95/max ms | engine P95/max ms |
| --- | --- | --- | --- | --- |
| Windows11 / 最终R2包 | 148.75 / 0→13 | 10.7166 / 11.1764 | 4.8829 / 6.2661 | 4.8551 / 6.2322 |
| Debian12 / 解压R2包 | 27.27 / 0→2 | 16.3457 / 17.3908 | 4.5128 / 5.2865 | 4.4347 / 5.1667 |

UI计时从TextEdit changed到当前结果的UI构建/提交前位置，含worker与UI轮询；不是物理屏幕呈现。worker CPU诊断为该线程区间总运行时间，包含其间其它工作，不是纯匹配CPU。分项分位数不能相加。Windows原门槛UI P95<50ms且worker max<50ms、Linux原UI P95<50ms均保留并通过；没有扩大截止或忽略超时。历史53.29ms和初次优化67.98ms仍保留，不能将最终有限样本外推成任意硬件/负载50ms保证。

最终R2真实GUI证据：`master-review/export-windows-r2/result.json`（verification_id `97637a08-b23e-4b14-b4bb-26b1060e0dcb`）及41个稳定进程句柄终态记录；`export-linux-r2/result.json`（`a411d4d9-fbb6-4fae-a60c-0cbbb5748a40`，实际程序/renderer位于`/tmp/mdcard-followup-r2.5jC2Te`）。两个环境均三取消+原生关闭、可正常重试导出HTML、程序exit0；Windows用HasExited，Linux检查PID+starttime及终态，未用单纯“进程对象存在”代替活态。截图中慢资源预览警告来自故意延迟的HTTP夹具；实际导出HTML已嵌入校验后的PNG，不把该截图当作普通预览全功能验收。

九格式每平台均实际执行HTML/PDF/DOCX/PNG/JPEG/EPUB/Marp HTML/PDF/PPTX。结构检查PDF2页/书签、Marp PDF3页/PPTX3页、DOCX TOC/书签/分页/12媒体、EPUB多级nav/6图片、PNG/JPEG严格解码与离线嵌入通过；Windows1316×2348、Linux1316×2344。主代理实看Windows长图与真实GUI截图，未声称逐页核验所有PDF/Word分页。双平台portable Node独立验收另覆盖错误/缺失/预取消/超时、三轮根子进程取消立即终态和重试。

完整R2候选位于 `artifacts/bug-audit/audit-followup-r2-20261022/`，不是已安装或正式发行；原dist两归档hash保持不变。

| 平台 | 归档 | bytes | SHA256 |
| --- | --- | --- | --- |
| windows | `MDcard-0.1.1-audit-followup-r2-20261022-windows-x64.zip` | 537040281 | `acb8c053d55bacce3021b0df1e72878c765a09bd02d604748b206b9ff5ea7cb1` |
| linux | `MDcard-0.1.1-audit-followup-r2-20261022-linux-x64.tar.gz` | 614192342 | `b74afdd9801316e26be4dc4dfdb59a8d5474a85778338b8f84aad3d8536a55cc` |

包内台账是5d5f290源码冻结时的待验快照；根目录本文件和`docs/releases/2026-10-22-bug-audit-followup.json`记录包完成后的验收，不回写已散列归档。机器记录包含最终源码输入身份、候选hash和证据清单。

保留边界：Linux导出需pidfd API/权限（启动前检查）；JS PID信号的身份核对不等同Rust pidfd原子保证；完全脱离父链/session/group的未知daemon不在捕获保证内，无法确认时报告cleanup错误。Z终态不等于已经由PID1回收所有非直接后代。Windows最初CIM样本没有当时内核态，未把它说成已证明的系统观测根因。未重测Wayland、真实IME/读屏、多DPI、物理断电；普通稿件崩溃恢复仍为既有能力缺口。本轮不宣称项目无其它Bug。
