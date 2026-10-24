# 文字精度阶段B：实现与验证记录

当前产品提交为 `40b5e3bb16e1658e333cf2fddf965a22d4e878ff`（R29），编译清单 `stage-b-r29-source.json` SHA256 `655cfdafcdd4feae4d3199206b49a39ba72c497966adec6378bcfe32d2210b55`。阶段A与B分别在约定支持范围内通过：实际代码/测试、双平台性能、完整包Win8/Linux11真实GUI与独立审计、运行前后全资源校验通过。最终结论与包SHA见 [交付记录](READING_PRECISION_DELIVERY.md)，完整原38项及全部限制见 [验收表](READING_PRECISION_ACCEPTANCE.md)。

## R29当前产物及验证

| 产物 | SHA256 |
| --- | --- |
| Windows应用 | `face40f4af716c004b39e4ce992af088dedda171f924a73df8cf8504eefb67e1` |
| Linux应用 | `b438bd9e042366a77c63bcb9f6ccc989414119788959086cec98114f2c6cd9cf` |
| Windows原生测试 | `5555163c48468d8e900ab66a579f4a5c8d8f7bb998949175adc0d75786ca9de1` |

82列入输入、70实际本地编译输入及5份dep-info已独立校验并绑定产品Git。Linux219通过/3忽略、Windows本机210通过/2忽略，search123/3忽略、perf125、fmt及双strictClippy通过，计数不合并冒充独立案例。实际运行有子进程HOME/XDG/AppData/temp/cwd隔离；忽略项明确保留，不声称已执行。

原264基线分别重建Win/Linux，功能代码只加入与候选相同的745f2725观察器和正常退出刷新钩子。48份final raw/24格式行的OFF/ON all/wheel P95原20%/2ms预算全通过，最终submitted=processed=total=len、flush_complete、0queue/retention逐份核对。六阶段原始数组与30次完成匹配独立重算通过；hot P95最大0.090382ms、单次1.969573ms，达2/8ms。线程CPU未测，source-index各n1、layout n2000尾窗及共享完成批次不加倍均明确保留，不称物理显示延迟或FPS。

R29仅在R25产品上改诊断传输：Linux reading/UI固定2句柄且真实dev+ino/readonly前后检查，Windows保持原stdwrite；rich诊断保留同目录临时文件原子发布；观察器自身原子快照及正常退出限时FIFO/ack/结束线程。所有产品诊断完整字段、每update频率、同步位置和实际计时范围保持，原稿/真实reading-state durable writer、纯定位器685a、状态d552、几何及源码映射均未改。新增必要的权限/删除替换旧inode/FIFO/缩短/失败恢复/并发读取/队列及确认超时测试有真实文件断言。

R29实际running取消及PNG/KaTeX异步稳定已新执行；基线72/72、候选73/73正常关闭smoke验证最终计数，未复用作正式性能样本。R25完整GUI矩阵、原冻结1400和768公开变体作为相同685a/d552/geometry组件证据明确单列；R29包内关键GUI已实际Win8/8、Linux11/11通过并独立审计，原旧矩阵与新包验收分别保留。R25/R26预算失败、R28空raw未验、R27调查与首次QA错误路径均保留。

## R25历史功能取证

下列R25原始记录保留，其中“当前”只指当时快照，不指R29完整包。

R25历史产品为已提交 `f579461cfcfc28797bce2f8b8b89e6c64742ae4f` 的 R25 冻结候选。R25 新纯定位器已通过独立1400、768公开结构变化、当前核心/Windows原生/严格检查；原大MD762647拆合与普通MD40557右改写两个真实Windows失败原位复验通过。双平台完整功能已通过，正式配对性能、完整包及包内复验仍在收尾，未执行项目不提前标通过。R24完整组的两个真实安全降级均保留，不能用此前763018单点通过代替。本文不代替主代理的 [独立B验收](READING_PRECISION_INDEPENDENT_B.md) 与 [38项验收表](READING_PRECISION_ACCEPTANCE.md)。阶段A独立提交与包见 [独立A记录](READING_PRECISION_INDEPENDENT_A.md)。

## R25源码与产物（历史）

冻结清单 `artifacts/reading-precision/stage-b-r25-source.json`，SHA256 `eb9f7309b73412ea3a025b4f38c3931b7978c264862209be1cb31781b5c5421e`。32源码及49资源为列入校验的superset；实际release/native/core/vendor rustc dep-info另列69个本地编译输入、5份原始dep-info，含六语种welcome及原生编码fixture。Cargo.lock与工具链另列，不称所有外部依赖。清单历史base_commit不改写，后续执行/提交绑定另列。

| 当前产物 | SHA256 |
| --- | --- |
| Windows `stage-b-r25-windows/MDcard.exe` | `75c819cbbd2bff9a0c786647dda99dfcad8006268df2f47ff573c7cf7270b6e7` |
| Linux `stage-b-r25-linux/mdcard` | `e8bb4de31266b5e519f110c489cc013a71a6142aac4eb39f3471fa0e5bfbe386` |
| Windows原生测试EXE | `67ee784e49105edaa35010ece84c4b1c28ce1527b24a3dc68abe4c79cf0f60de` |

当前纯定位器为 `685a8475a8cf8314c05ae8bbf7a72ffdf099201613a9e992c8935366919f1af9`。R25只改变此模块：完整旧node在已验证scope内唯一时，不再被全球重复的较短context窗口计数挡住；将完整旧node、已保存邻节点片段及实际相邻节点之间至多8B原始空白，按原坐标拼成有界literal corridor。每个字节必须已有证据，缺口或矛盾重叠即拒绝；原新两侧全局唯一见证与所有最优差异路径一致仍为硬约束。较弱interval-only分支保留旧context-count和直接邻块门槛。没有增加字段/schema或改变32字素seed、8KiB proof、4KiB节点与DP预算。旧40557记录未保存远邻间隙时安全降级；同原稿重新capture可文字精确，不从缺失旧证据推断原文。

R24短heading仍要求真实旧/新parser节点身份、完整源范围及全局唯一，不能只看字符串以#开头。R21首次源码索引挂载保护、实际安装解析版本绑定、失焦选区与存储合并保持；R22只在pending且目标Galley缺失时用本帧可见mapped源范围反馈挂载；R23的量化停止余量仍为一物理像素加两个egui GUI_ROUNDING坐标步，并夹在0.75–1.25逻辑px，实际验收仍2px/120步。估算挂载不称exact，最终以本帧真实目标Galley核对。

历史布局失败已按原字节复验：普通MD隐藏长链接的45884→115252未夹取y误差0.71875px、3补偿、2.1s稳定；原文末58240为明确document-end夹取87.625px、4补偿，单列不计未夹取分母。完整原稿SHA、失败及实际截图保留。

## 实现与资源边界

阅读锚点使用实际可映射的原文字素与本帧 Galley 几何，独立于光标和选区；文字、源码逻辑行、块、章节、近似、无效等级分开，夹取单列。Read↔Split 保持 preview 自己的锚点，Write↔Split 保持 source 自己的锚点，不把原本不同的视图历史强制同位。源码 logical_line 依原稿位置独立校验，visual_row 与底栏行数不作为文字命中答案。

内容改变后按 buffer/revision 的 Arc 正文快照补全有限字素上下文、标题、完整原节点身份、邻节点及实际原/新唯一见证，再进行有界、所有最优路径一致的差异映射。两端固定强制对应可折叠，每个固定相等字素的 4 分仍加回完整总分，hunk、门槛和源坐标按完整 scope 计算。自动化穷举验证候选/分数多重集合及歧义结论等价；目标始终保留在动态区间。证据不足、原锚删除或超预算按存活邻块→唯一章节→近似位置→无效降级，落点新文字不把旧锚决定提升为 exact。

| 边界 | 当前约定 |
| --- | --- |
| GUI 原文件打开上限 | 16 MiB，保留原保护 |
| 纯定位器内部解码正文/节点预算 | 64 MiB / 50,000 节点，包含真实逻辑行；不代表 GUI 支持上限 |
| 动态差异旧/新侧 | 1536 / 4096 字素 |
| 单锚 proof / 完整小节点片段 | 8192 B / 4096 B；不是随全文增长的历史快照，极短文档可能被有界片段完整覆盖 |
| 持久化 | 512 条 / 4 MiB、后台 OS 锁合并、原子替换；与 Markdown/恢复稿/替换历史分开 |
| 平台证据 | Windows 11 原生与 Debian12/X11/Xvfb/Openbox/Mesa；不扩大到 Wayland 或所有 Linux 桌面 |

schema3 默认独立 `reading-state-v3.json`，首次缺失时只读迁移旧 v1/v2 默认文件，旧文件保留。未来版本、损坏及非法字段拒写保留原 bytes、内存和可查看提示。显式强制旧二进制写入新路径属于不支持配置。清除是锁内显式事务；真实输入与后台元数据排序分开。单清墓碑被淘汰后，全局有界 clear 序号/时钟仍限制 missing key 的旧缓存，bind 先丢落后历史，再允许真实新活动；存在的未清 B 不受其他文档单清影响。

R17 界面摘要只在实际正文 revision、成功保存基线或待保存格式变化后重算。clone 的缓存独立；真实 undo 回到 saved 内容仍显示 clean。保存、关闭、切换与外改保护继续调用真实全文比较的 `Document::dirty`。普通 RichUi 和 Outline 用 BufferId/内容 revision 门控，异步 poll、debounce、controls、布局仍逐帧处理。源码排版对普通≤64KiB正文或虚拟窗口的行/hash/字体缓存检查另计，不承诺所有第三方绘制 O(1) 或零遍历；显式保存、关闭和搜索命中等动作仍做真实完整检查。

## 已执行自动化与独立算法证据

| 当前检查 | 实际结果 |
| --- | --- |
| Linux核心 | R25：206通过、3明确忽略，实际exit0 |
| Windows本机原生EXE | R25：199通过、2明确忽略，实际exit0 |
| Linux / Windows strict Clippy，fmt | 通过 |
| search harness | R25：123通过、3明确忽略，exit0 |
| performance harness | R25：125通过；旧R19首次118通过/1次finish发布观察竞态与随后串行119通过均保留，R20并行119通过 |
| 新界面定向测试 | 实际 egui TextEdit→undo→rich patch；Unicode计数、保存/格式、clone/commit；TOC同ID同revision不同buffer/空结果；rich debounce/undo/每帧poll通过 |

上述 harness 导入同一产品模块，计数不能相加。主代理还用独立公共 API 验证缓存 clean 且故意未通知 revision 的同长度改稿：真实 save_to 仍写新 bytes，之后外改冲突拒绝保存并保留内存及磁盘，见 `stage-b-r17-independent-save-cache.log`。

新纯模块 `685a8475` 已由主代理fresh重跑独立gate：8项精度/实际Galley、11项公开结构测试共768变体通过（原228+边缘288+常见内部/丢右邻ID216+跨node改写36，不加入1400分母）。新1400仍1000/1000可判定文字exact正确，200歧义和200删除全降级，错误exact/超时/崩溃/缺失/非法边界均0，见 `evidence/reading-precision/master/stage-b-r25-locator/score.json`。原始输出SHA `9f88dd45bb5c84fdd6503098d8fbff05c66e32d120cd7025a35ac852cfb5c725`。lib33/structural34含导入产品模块的重复测试，不能与独立8/11、1400或768变化相加。旧0f3成绩不复用到新算法；输入/答案在实施前冻结，533例编辑量≥30%、300例块移动≥30%。有限独立集结果不是任意原稿零误定位保证。

当前R25源码的主代理独立完整 gate 为118通过/4明确忽略，随后显式串行4项持久化公共API测试覆盖14变化全部通过：future8、同实例新活动2、墓碑实际淘汰后metadata/rebind2、真实重命名/先清邻稿2。未清B900、新阅读450、512/4MiB及原稿保护均核对，不能把导入测试计成独立总数。

## 当前Linux真实GUI（R25：11+2通过）

R25 Linux11组功能及2组严格R03导航竞争全部通过，冻结 `artifacts/reading-precision/implementation-r25-functional-complete.json`，SHA `a187c56bdb04536ba73486ced9c7362b8dbc3770910c6194afcfc5040424f5d4`。13结果、15个实际driver/helper/runner的精确副本及SHA、74张功能PNG与R03的PNG/原始JSON均绑定；导航在启动时另保存实际exec的driver与helper前缀。当前六张关键实图（PNG/公式前后、原heading撤销、equal-revision双buffer）由实现侧实际查看，其他图由独立验收按计划复核。组数不与独立集或Windows断言相加。

| 当前Linux真实GUI | 证据与边界 |
| --- | --- |
| 副稿/同文件双buffer/原heading撤销 | 普通副稿文字与非零失焦选区、比例/wrap/切稿/重启；同DocumentId/同revision不同正文的实际字素独立；原153885“操”撤销至49485，实际绘字/焦点/选区正确 |
| Read/Write各自重排、四格式中部拆合 | Read↔Split与Write↔Split分别保持各自原文身份/y；普通/大MD/TXT中部真实滚动、独立splice与2.1s稳定 |
| 真实图片/公式异步 | PNG offset400→1476、byte1441、最大y误差1px/1补偿；KaTeX400→619、byte1064、y误差0/2补偿；加载后2.1s停止补偿，实图已查看 |
| R02继续编辑/切视图 | 自然运行的版本匹配中真实source输入到新revision；实际pending时切Read，旧任务与独立视图历史不串用 |
| future/clear/两实例/容量 | single/All、合法未来元数据、真实新活动与重启、旧proof完成不复活；容量使用CLI公共API512次setup，非512次GUI打开 |
| R03 Search版本恢复优先 | 外改10.12MB→13.30MB；旧source历史28000，真实Search点击使运行中match被取消，实际“N”字素11612441..11612442；宽变后2.3s稳定 |
| R03 TOC版本恢复优先 | 10.12MB稿在原生source中Paste65540B，首标题byte0不变；旧历史与pending原字素仍28000/offset1439.9995，真实目录click取消当时运行的rev1 match，最终实际“#”0..1/y0；真实保存正文SHA与独立构造一致，宽变后2.3s稳定 |

两组R03在生产任务 `cancel_requested_while_running=true`、phase=relocation-match后核对任务实际结束、当前Galley与原稿版本；不把点击前running代替处理时资格，也不称100ms性能。Search与TOC均使用driver `8dae7ab20b5e441a01908c24613c39132ae30598272df48c2ad6d4110273356a`、实际helper `f1d13decd60b03a838fb9ab19ed82ea412689ba9c19c933129da1ee3cd214a6c`，真实宽变后的尾段分别2388/2498ms。Search的visible_text诊断logical_line=0为默认值，不用于验收；原文LF独立计数与活跃geometry均25504，TOC均0。

当前容量真实复验512条/551826B，A旧640、B959.99994；原8秒内先验证两文档实际view偏移及锚点落盘并存不可变seed，单清前核对enabled/hovered、owned PID/active window、实际pointer落在同一rect以及frame19。随后实际墓碑淘汰、旧metadata不复活、A重新打开0，再真实新滚640.00012并重启同值，B保持，原稿不变。CLI512次公共API setup不称512次GUI打开。当前disk_observation_errors为空；历史初读ENOENT与误点All失败均保留，不追认根因。

R24完整Windows组又出现不同真实位置：大MD762647“接”拆合、普通MD40557“c”右改写均安全降级，原稿/独立splice/oldproof/失败实图保留。R25这两点已按相同原字节和old/new SHA真实Windows复验文字精确通过；第三个历史763018原点与完整23组另由主代理执行。Linux R25的11组及严格R03按新产物全部通过；Windows当前23组已完成：原R25a的largeMDWrite在8/10后因复用观察文件清理占用失败，旧PID正常退出、占用原因未被证明；独立唯一report路径R25b完整10/10通过后合并为23组通过。原失败不追认whole通过。R24 Linux10原组+单独R24b容量协议通过，但R03 Search在GUI结束后诊断结果读取临时position=None而失败，异常截图又因窗口关闭无超时挂起；原失败保留，QA改为优先pending.position、截图限定8s且关闭窗口不拍，不能把旧case追认whole通过。旧formats仅第一相交块不同的失败，经主代理实际同4023“M”/完整源范围/三宽几何独立核对最大误差1.375px；只更强QA核验，未改产品或容差。 R24另两项Write控件等待/进程提前退出失败，根侧同二进制附stderr/exit观测重跑52重排+20生命周期均通过，原因未被证明；历史失败保留，不统一归为输入驱动，也不据此追认整批通过。

先前R21的11组完整原生Linux证据仍保留在 `implementation-r21-functional.json`（SHA `50c16bfe1ebbeb7eb9a70c9409fab8349297a52c420b00182889f4bf83813acb`），包括普通副稿/原heading跨large回普通/同ID同revision不同buffer/异步PNG和KaTeX/各自Read或Write与Split/四格式中部拆合/R02编辑竞态/未来时间清除/两实例容量清除。它们是历史组件回归，不能代替当前R25整体GUI。

核心与原生测试的最终执行使用显式 `/work/Cargo.toml`及实际dep-info，Linux子进程所有XDG、settings/reading、cwd/temp均隔离到全新原生 `/tmp`；Windows原生测试使用child-only LOCALAPPDATA/APPDATA/TEMP/TMP及settings/reading隔离。此前未完整记录env的日志保留，不事后追认隔离。R22在 `/work` 出现的codec立即读回ENOENT，与同SHA ELF在原生 `/tmp` 单项通过均保留；一次重试不作为文件系统根因证明。artifact调试共享Cargo target的缓存冲突经SHA发现后失效app-test fingerprint，恢复相同R22 ELF；最终R23显式重编，artifact后续使用独立target，不把调试副本作为最终源码。

## 最小补充验收（当前R25）

真实自然运行的enrichment/wheel、matching/wheel、matching/tree三阶段均在处理输入时仍运行并实际退出；native到读到实际退出的墙钟上界分别31.9327、38.9036、62.1356ms，token到退出分别0.0133、0.0218、1.7929ms。新小稿62.1356ms已可用，新任务排队0.0417ms；无人工任务延迟。三阶段/proc VmHWM最高769636KiB仅为该app进程历史峰值，不含renderer、不代表配对额外内存。

独立headless crate使用真实GeometryCache和12个60000余字符的实际egui Galley，始终≤2MiB、最终1920640B；保留所有Galley Arc时最旧索引被淘汰并重新构建，13次构建后连续40次同Galley命中不再索引，实际e+组合音标源范围保持。Cargo.lock、rustc、构建命令、ELF和dep-info封存，不将headless压力称GUI或分配器峰值。

原生Linux新/tmp子HOME/XDG/cwd/temp中，旧A包实际schema2与新R25均不设reading override：旧真实滚动799px只读迁移至v3后dx/dy0；旧默认writer并行及清除后再写均保持旧v2与当前v3分离；当前真实ClearAll、重启后0.2353px，再真实新阅读400.2353px可保存，原稿SHA不变。旧A包manifest/二进制已先核对。首次Read即时点击未切换的setup失败及精确旧driver保存，fresh阶段使用跨帧native点击而非状态注入。证据统一为artifacts/reading-precision/implementation-r25-supplements.json；这三项不混入正式性能分布。

## 性能状态与历史失败

当前R25 Linux独占正式12run与新685a completed-stage已全部exit0，23:14:12–23:19:24 UTC完成配对，之后专用后台完成基准结束；进程退出已检查。正常OFF三尺寸两格式的完整update与wheel P95增量全部通过，hot六组P95最高0.088701ms、单次最高3.133585ms，满足预冻2/8ms。同步ON普通MD/TXT wheel分别15.986139→21.764383ms、16.683884→20.745884ms超过原20%/2ms预算，明确失败；ON1/10MiB当前比较通过，不以此抹去普通档或历史失败。Windows当前正式12run由主代理独占执行，本文不提前填其结论。预冻基线为264d42d加相同完整App::update入口观察器；普通/1MiB/10MiB、同输入/窗口/字号/flags，完整P95增量≤max(旧20%,2ms)，hot P95≤2/max≤8ms不变。正常diagOFF为预冻主结论，同步diagON单列；不能用OFF掩盖ON失败。

历史R15双平台全部OFF比较通过，ON失败完整保留：Linux普通TXT wheel17.046227→23.463450、1MiB MD20.686576→27.885772；Windows普通MD7.4880→10.3375、1MiB MD30.9127→72.9511、10MiB TXT12.5571→73.8566ms。完整all/frame/逐例样本分别在 `linux-final-r15-perf`、`windows-final-r15-perf`，不选择较优历史数据代替最终R25结果。

完整update包含产品同步诊断I/O，入口observer自己的异步入队另列。Instant是函数墙钟，非线程CPU、物理显示延迟或FPS；字体排版、索引、主提交、后台磁盘分阶段报告。普通滚动matching n=0；当前685a专用后台各尺寸enrich/match均n10实际完整成功：enrich P95为4.417587/38.404992/325.190009ms，match为0.9828/18.073012/177.616717ms。与actual GUI completed source-index各n1、捕获、按类型实际排版、主线程提交及成功磁盘事务的P50/P95/max/n分别列在linux-final-r25-perf/six-stages.json；单个index样本不称稳定尾分布，同一纯match分布并列两格式时不重复计数，取消样本不混入。当前Linux比较/六阶段/原始SHA及资源边界封存implementation-r25-performance.json。6.525s实际持续滚轮中成功落盘2–3次；两app VmRSS 50ms采样峰值基线/候选分别为普通375324672/379736064B、1MiB443633664/450678784B、10MiB1151385600/1187045376B，均非renderer总量或分配器峰值。原始历史继续保留。

| 原失败 | 处理与留存 |
| --- | --- |
| R5拆合尾部映到前一重复段，错误exact | 原现场保留；完整旧scope/唯一见证强制映射，独立负例及原字节GUI已复验 |
| R6–R14存活目标仅块级、三次重复节点中部降级 | 不换锚/删样，最终034e通用边界与228变化/四格式实际对应复验 |
| R11/R12合法future元数据压过clear、同实例sent戳抑制新活动 | 独立clear事务/真实活动/单调metadata修复，公共API和真实双实例/新活动复验 |
| R15容量淘汰单清墓碑后旧metadata复活 | 实现和主代理分别真实失败，05f5全局missing-key屏障修复，12变化及真实容量GUI通过 |
| R16/R17 large→ordinary撤销时旧节点套用新正文几何，原heading丢失 | 绑定实际已安装解析版本，等待时不采旧几何/不完成恢复；R21原153885→49485“操”真实复验通过 |
| 普通副稿缺少文字几何；失焦toolbar wrap后非零选区归零 | 副稿接入实际Galley/原文映射，失焦读取持久TextEditState选区；R21 ratio/wrap/C→A/重启实际验证通过 |
| 内部rename同会话身份保持但重启新key丢失 | 同锁显式迁移、新path强制提交及queued seed/同毫秒失效修复；当前独立14变体含正常/先清邻稿两真实文件重命名分支 |
| R19 largeTXT副稿同布局重开raw offset漂移4.7562256px | 固定ppp1.25/font14.6/width405.40625实际show_source复现临时line0导致5.0625px内容原点变化；R21保护首次挂载。主代理真实Windows同原byte889/同layout的C→A rawdy0、重启rawdy0.0324097、文字y误差0.306px。该42断言子序列已过；末尾新增samebuffer的驱动前置失败与重跑另列，不将整体原failed报告改成通过 |
| R23大MD章节末段首，右邻短标题证据被24B过滤，拆合后降块 | 原763018→763019及完整old/new SHA失败保留；短seed不完整实验保留；R24使用实际短heading完整节点/原新全局唯一/完整scope，独立516补充+新1400和原位真实Win已过，整批仍待验 |
| R24大MD常见段中window被截断/普通MD右改写删除下一nodeID，分别安全降级 | 原762647/40557、原稿/变换/oldproof保留；R25完整node唯一性与无缺口旧literal链修复，768公开补充/新1400及两原位Win复验通过，整批仍待验 |
| R17初稿Cell无法满足搜索索引Sync | 编译失败日志保留；最终buffer本地Mutex保持线程共享能力与clone隔离 |
| 历史同步诊断ON超预算、驱动setup/输入/观察不足 | 原始数据保留并分类，不改阈值、不以较优候选覆盖、不追认通过 |

此前逐轮实现叙述已归档为 `artifacts/reading-precision/implementation-report-history-through-r17.md`；全部原始artifacts/evidence继续保留。

## 待交付门槛

当前Windows独占正式配对、最终QA/报告提交、完整包逐成员路径/大小/SHA/源码提交/资源核对，以及包内字体、异步内容、大改重定位、clear与R02原生复验尚待完成。最终包明确指定包内renderer，不回退旧dist。未完成项不得提前标通过，未验Wayland/其他桌面/任意内容保证明确保留。
