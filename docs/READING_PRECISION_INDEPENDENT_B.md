# 阶段 B 独立验收与交付记录

状态：**阶段A与阶段B分别在约定支持范围内通过。** 当前产品40b5e3bb、包快照8c09f82、Windows face40f4/Linux b438bd9e；82输入/70实际/5dep-info/3bin与Git已独立绑定。两平台48份完整raw/24格式行原预算及六阶段审计通过，原冻结1400为1000正确精确/400降级/0错误精确。最终完整包Win8/8、Linux11/11真实GUI及独立原稿/实际坐标/资源归属通过，运行后22059/22003全部文件与清单一致。独立结论、全部包SHA和限制见 [最终交付](READING_PRECISION_DELIVERY.md)；历史失败继续保留。

原始38项标准完整保留于 `READING_PRECISION_ACCEPTANCE.md`；精度与实施前冻结预算见 `READING_PRECISION.md`。阶段 A 证据及两个完整包见 `READING_PRECISION_INDEPENDENT_A.md`、`READING_PRECISION_STAGE_A_PACKAGE.md`。以下逐轮取证过程保留当时的“当前”“待验”等原始措辞；当前最终结论以上方及 `READING_PRECISION_DELIVERY.md` 为准，不用后来的通过追认旧轮次失败。

## 独立对应关系评测

测试集在实现前冻结于2026-10-23 01:53:48 UTC。1400个不同旧版—新版文档对，每对一个锚点：1000可判定、200证据不足的歧义、200删除/无对应。答案来自独立 Unicode scalar 编辑溯源，未交给实现代理，不调用产品匹配器。533例字符插入量加删除量达到旧文scalar长度30%，300例移动原块达到原块数30%；具体操作和分布均保留，不用UTF-8字节夸大中文编辑量。 全集字符编辑比例min/P50/P95/max为0/11.50696%/100%/101.18271%，可判定1000例中300例字符编辑量≥30%、300例块移动≥30%；可判定字符编辑比例P95为73.27303%。类别及分位数原始汇总见 `evidence/reading-precision/master/b-frozen-change-distribution.json`。

| 指标 | 结果 |
| --- | --- |
| 正确精确恢复 / 可判定样本 | 1000/1000，100% |
| 自动精确恢复覆盖率 | 1000/1000，100% |
| 精确声明正确率 | 1000/1000，100% |
| 错误精确声明 / 全部样本 | 0/1400 |
| 歧义降级；删除/无对应降级 | 200/200；200/200 |
| 全集降级率 | 400/1400，28.5714% |
| 超时、取消、崩溃、非法边界、缺失结果 | 各0，保留全部分母 |

该结果仅描述本轮独立集，不承诺任意文稿绝对零误定位。另有8项独立原稿/实际Galley精度测试及11项结构测试、原228变体及288段落边界、216共同文本、36跨节点改写变体通过；不加入1400统计分母。新增288+216+36来自公开真实GUI失败后的独立回归补充，不冒充此前未见的验收集。全部逐例结果、400降级、候选证据及空失败清单在 `evidence/reading-precision/master/stage-b-r25-locator/`；本次运行日志在 `artifacts/reading-precision/master/r25-independent-gates/`。结构测试源码为 `tests/reading-precision-acceptance/tests/relocation_independent.rs`。

按纯定位器原始等级分开统计：文字级1000、源码逻辑行级0、内容块级100、章节级0、近似位置250、无有效位置50，见 `evidence/reading-precision/master/b-r25-locator-precision-tiers.json`。零计数不宣称该等级已命中；该纯算法统计也不含GUI坐标误差，实际文字级通过还须满足对应GUI字素与≤2px合同。

| 冻结或被测对象 | SHA256 |
| --- | --- |
| 独立输入 | `980f8136cc87edb9bcca12a4beecd5e7cec12c64fdcb64689d9d88805f5d69f5` |
| 独立答案 | `2cdeac356af1b00545f38d79eddc5c60a6d7135ed66f40e22849e1cdc73c08a0` |
| 评分器 | `1534350d287bfadfd5304aaa0827cc6b55d4b29dbb751a760b095c3a5084bec2` |
| 运行前后相同的纯定位模块 | `685a8475a8cf8314c05ae8bbf7a72ffdf099201613a9e992c8935366919f1af9` |
| 独立评测二进制 | `4807e30cdc4f425cedcb5c33ac7c7da945187def68387a5227caa1782a6b8ea9` |
| 全部原始结果 | `9f88dd45bb5c84fdd6503098d8fbff05c66e32d120cd7025a35ac852cfb5c725` |

R25改变纯定位模块后，主代理用原始冻结输入/答案/评分器重新编译适配器，完整运行1400例，没有复用034e或0f3成绩。capture P50/P95/max为0.413856/0.671523/2.005848ms，resolve为0.146510/11.571371/12.854074ms，各n=1400，属于算法墙钟，不是正式GUI性能、线程CPU或显示延迟。评分器输出目录名为score.json，汇总实际路径为 `evidence/reading-precision/master/stage-b-r25-locator/score.json/summary.json`。

## 精度和支持边界

阅读锚点、源码阅读位置、光标、选区分别记录。文字精确要求原稿对应字素身份正确且实际Galley视口y误差≤2逻辑px；同布局xy分别≤2px。源码逻辑行、视觉行、字素分开核对，不以行号、比例或相似段落代替文字身份。夹取单列原因和实际误差，不计入未夹取精度分母。

内容变化使用有界原文、上下文、标题、邻块和可验证差异。重复候选缺乏区分证据必须降级，不选首个、最近或百分比相近候选充精确。文字、源码逻辑行、内容块、章节、近似、无有效位置分别报告原因。隐藏语法、图片、公式、图表及生成节点无法映射时降级，不冒充原稿文字。

旧/新差异区间预算1536/4096字素，持久化证明8KiB、节点证据4KiB；超预算或证据不足明确降级。源码索引上限为解码后64MiB/50000节点；GUI仍保留原文件16MiB打开上限，不能声称可打开64MiB文件。草稿独立状态不代表草稿正文跨重启恢复。

## R25功能矩阵取证快照（历史构建及相同组件）

该完整功能矩阵在产品提交 `f579461cfcfc28797bce2f8b8b89e6c64742ae4f` 执行；此节“当前”指当时构建，R29实际产物见上方及本节末的当前绑定。冻结清单 `artifacts/reading-precision/stage-b-r25-source.json`（SHA256 `eb9f7309b73412ea3a025b4f38c3931b7978c264862209be1cb31781b5c5421e`）列入32源码、49资源及3二进制；其中69个本地输入由5份实际rustc dep-info确认。主代理逐一绑定当前Git与原始字节，见 `evidence/reading-precision/master/b-r25-commit-source-binding.json`、`b-r25-dep-info-independent-audit.json`。Windows应用SHA256为 `75c819cbbd2bff9a0c786647dda99dfcad8006268df2f47ff573c7cf7270b6e7`，Linux为 `e8bb4de31266b5e519f110c489cc013a71a6142aac4eb39f3471fa0e5bfbe386`，Windows测试二进制为 `67ee784e49105edaa35010ece84c4b1c28ce1527b24a3dc68abe4c79cf0f60de`。原R21清单遗漏的welcome/native夹具独立补审及原manifest保留，不能追认原47条覆盖全部。完整包与当前最终GUI矩阵待验，不用中间包代替。

Windows平台为Windows11 26100 x64、125%DPI，i7-13650HX、RAM16,849,256,448B，实际运行报告OpenGL/RTX4060LaptopGPU。本轮硬件清单为 `artifacts/reading-precision/master/windows-environment-r25-before-performance.json`，硬件枚举本身不作为实际renderer或性能证明。原生键鼠仅操作所属HWND/PID，JSON只读观察，另核对原稿范围、完整字素、实际坐标及桌面截图，恢复后至少2.1s稳定观察并记录补偿次数。

Linux范围为Debian12、X11/Xvfb/Openbox、Mesa软件OpenGL，不代表Wayland、所有桌面或硬件GPU。Unicode坐标验证不扩大成全部emoji彩色字体呈现承诺。真实GUI与批量算法评测分别验收，互不替代。

## 持久化与保存保护

继续采用独立状态、后台临时文件及原子替换、512条/4MiB、锁内按文档合并、单个/全部清除和可查看错误。失败保留内存与上一有效记录，不修改Markdown、原稿恢复或替换历史。

R16修复单清墓碑被512条新文件淘汰后旧实例复活位置：记录池外保留clear时钟，缺失文档重新绑定前同步，真正新阅读仍可持久化。R16模块及12变化、R21默认114项和4项持久化专项均为历史证据。当前R25 state模块SHA为 `d5523f6ed87a5663682a448f4d15011c04e3edce053b5d1f732aebb43c693ac4`，根侧在显式child reading及原生Linux `/tmp` XDG/state/config/data/cache/temp隔离下，重新执行118项默认与4项专项/14变化，参数、测试ELF及日志哈希在 `artifacts/reading-precision/master/r25-isolated-independent/execution-manifest.json`。专项核验实际rename当会话及重启身份、位置与选区，不用人为等待避免同毫秒回归。Linux当前容量为512条/551826B，新A640.000122重启相同、B959.9999389保留；512文件setup由公开写入器实际打开/记录，再用原生GUI验不复活及新滚动重启，不冒充512次GUI打开。

R17主代理独立保存测试在已缓存clean后故意不更新revision地等长修改正文，实际save仍写正确bytes；外部冲突拒绝覆盖，内存及外部bytes保留。日志 `artifacts/reading-precision/master/stage-b-r17-independent-save-cache.log`。真正save/open/close guards继续权威正文检查。

## 性能合同与最终测量

R29双平台最终配对及原始数组审计已通过，R25/R26及R28原失败按历史保留。冻结预算：热捕获/计划/入队P95≤2ms、max≤8ms；完整App::update P95增量≤max(原基线P95×20%,2ms)；实际计算中原生取消可见上界≤100ms。字体排版、后台索引/证据/匹配、提交与写盘分别列P50/P95/max/n。

旧基线264d42d使用与最终候选相同的update_probe，保留源码归档、观察补丁和二进制SHA。普通、1MiB、10MiB、长段落和重复文本使用同机同输入SHA、字号15与相同真实滚轮序列。Windows nearest-rank、Linux上侧排序索引分别在平台配对内保持一致，不跨平台汇总分位数。RSS采样不含renderer子进程，也不是分配器峰值。

完整入口计时包含产品工作与既有同步诊断I/O；旧rich.scrollCpuMs仅主段墙钟，不称整个update或CPU。诊断OFF为实施前冻结的正常产品主结论，ON单列限制与全部失败。R15正常双平台配对通过，ON仍超预算，属于历史而非R17最终数据。

R17界面脏标记/计数、普通预览和目录按buffer/内容版本复用，避免应用外层每帧全文比较、计数、目录解析。显式保存/关闭仍真实检查；源码实际排版读取普通≤64KiB正文或大文档虚拟窗口，egui内部可能读取输入，排版单列。不能称全部第三方绘制零遍历，不为恢复强制挂载全文。

## 历史失败保留

| 原始问题或证据不足 | 处理 |
| --- | --- |
| 子像素循环、异步顶部覆盖、相对图片路径 | 原始失败保留，阶段A新构建逐项修复复验 |
| R10/R11拆合证明不充分、四个原Windows位置降级 | R15完整邻块范围与强制字素映射修复；四个原位置真实细滚轮复验 |
| R14长UID差异超预算 | 144种独立双侧拆合先失败；保持预算与样本后修复，R15全部228变体与1400通过 |
| R11未来时钟清除复活；R12同会话新阅读被过滤 | 分别修复clear顺序及单调新活动，失败与后续真实GUI均保留 |
| R15容量淘汰墓碑后旧快照复活 | 实现及主代理公共API独立复现；R16独立12变化及Linux容量GUI通过 |
| worker自然完成、树行离屏、点击协议不足 | 保留驱动失败或证据不足，不计作取消通过 |
| PowerShell读取/解析导致127–330ms观察上界 | 原记录保留；独立C#事先启动观察，R15实际运行期原生取消70.8637ms，不追认旧失败 |
| 多轮诊断ON超预算 | 保留所有原始分位数、最大值和失败比较，按既定合同分别报告 |

完整旧时间顺序叙述保留于 `artifacts/reading-precision/master/b-interim-narrative-through-r16.md`，SHA256 `81ed8b79935833448b485d6522d4e71f740c7abb84cec166ddbceaa270061675`；所有原始evidence/artifacts保留。

未验证平台、特殊内容、夹取、证据不足和超预算限制继续列出。阶段A、B提交、GUI、性能及包结论分别核对，不用块级结果或A完成替代B。

## 后续审计发现的真实缺口

- R17普通副稿20700B，真实18次滚轮后分隔拖宽407→527px，geometry/pinned/text_anchor均为空，不能由粗offset相等宣称文字精度。原失败stage-b-r17b-secondary-gap保留，R19接入实际Galley/修复失焦选区，R21当前Linux及Windows44项完整序列均已复验，最终包另验。
- R16完整Windows inapp组从900段前缀的大文档撤销回普通后，main/preview超过25秒无geometry；relocation曾声明精确，但actual_geometry绘字与source_text不同。原始证据precision-b-final-r16-inapp-lifecycle-read保留，须验证实际已绘制版本就绪后才采几何及完结恢复。

当前R25四格式两视图生命周期各20项、共8次异常退出：位置记录实际年龄1703–2168ms，最近成功写入完成年龄732–1445ms，见 `evidence/reading-precision/master/b-r25-abnormal-persistence-age.json`，不能称绝对零丢失。R23年龄1710–2192ms及旧副窗格/heading误差统计分别保留为历史证据。分组断言和重复观测不能当作独立文档数或1400批测样本数。

R21本轮失败：`precision-b-final-r21a-boundary-boundaries-read`（3个empty/nontext子项通过后文末失败），`precision-b-final-r21a-external-md-lifecycle-read`（首个prefix-insert失败）。原稿映射均为正确的粗体“粗”；因此不归因于纯定位器误选，但视口精度和边界判定仍不达标，不能用正确字素身份或安全降级宣布文字恢复完成。当前候选完整矩阵原始结果继续保留，不调2px/120pass门槛。

R22引入实际可见源码范围挂载反馈后，旧字号测试出现0.8125px量化振荡，虽小于2px仍未有限结束，原Linux/native失败保留；R23按物理像素及egui GUI舍入量设置0.75–1.25逻辑px内部停止余量，仍严于2px合同，四ppp旧实际preview回归与全套测试通过。Linux `/work` Windows绑定目录曾发生codec及持久化即时读取ENOENT，日志保留；同ELF在原生Linux `/tmp`隔离通过，不单凭重试追认根因或所有文件系统可靠性。

R23原失败定点的根侧独立原稿/字素范围、实际坐标及截图审计为 `evidence/reading-precision/master/b-r23-original-failures-independent-audit.json`；对应current Windows纯未夹取误差0.71875px和文末夹取87.625px分开统计。

新增当前失败 `precision-b-final-r23a-external-large-md-lifecycle-write`，result SHA256 `0b6e6ea3f0135cda58da50627c01e865420cff665b9d4b2faa331607755466b6`：prefix-insert/delete/move前三类通过，split-merge原段首763018→763019被安全降块到761650。独立原稿操作仅3 scalar变更；固定32字素节点seed跨越新增换行，短邻标题缺少强证据。不得把安全降级当精确命中，后续通用正负例与原1400分别验证。

R24根侧独立状态测试在显式Linux原生`/tmp`的XDG/state/config/data/cache/temp隔离下，116项默认测试与4项显式持久化专项通过；4项包含14变化。执行参数、环境、源码及测试ELF SHA见 `artifacts/reading-precision/master/r24-isolated-independent/execution-manifest.json`。旧无短标题证明的记录不会被补造证据，仍安全降级；schema与证明预算未放宽。

旧R23格式GUI的mapping.md返宽断言失败4020→3700，保留原结果。独立追加wide/narrow/back截图和实际坐标后，同一原稿4023..4024的M始终存在，真实y为16.40625/17.40625/17.78125px，最大变化1.375px；这是第一相交块在边界变化，不是文字误定位。格式矩阵因此补充原稿SHA、独立完整字素边界、相同原稿范围和实际old/new Galley≤2px的更强验证，不能只以应用exact标志通过。补充驱动首次把reading report的path误当文档路径，失败保留；更正使用所属窗口document_path后，R24该文件宽度、分屏和重启完整通过。最终全格式矩阵仍需在当前候选中完成。

R24留下两处真实失败：`precision-b-final-r24a-external-large-md-lifecycle-write` old762647..762650“接”split-merge应对应762648..762651却安全降级；`precision-b-final-r24a-external-md-lifecycle-read` old40557“c”rewrite-right降级。主代理分别以TargetSourceByte实机定点重现于 `precision-b-r24-repro-large-interior-lifecycle-write`、`precision-b-r24-repro-ordinary-rewrite-lifecycle-read`。旧763018已过用例不是同一目标，不能替代。原pure0f3b16的失败与调查保留，R25再用下述通用证据链修复，未放宽proof/DP预算。

R25恢复的是已经保存的原文字节证据：完整节点的唯一性不被重复短window计数覆盖；相邻旧节点与原空白必须能无缺口、无冲突地重建，才进入原all-best差异映射。未保存的旧间隙不猜、不以新节点文字补造旧证据；原ordinary旧proof可安全降级，同正文在新版本重新采集后精确通过。8KiB证明、4KiB节点、固定DP/seed和schema边界未扩大。

R25主代理独立118项默认与4项显式持久化测试通过，原生Linux `/tmp` 的XDG/state/config/data/cache/temp均明确隔离；源码/测试ELF/参数见 `artifacts/reading-precision/master/r25-isolated-independent/execution-manifest.json`。三处真实GUI原稿SHA/字素/逻辑行/实际rect与2.1s尾段独立审计在 `evidence/reading-precision/master/b-r25-original-regressions-independent-audit.json`。

状态测试计数说明：独立工程默认运行的118项中，主代理独立编写12项，其余106项是导入产品模块的原有测试；另外显式运行4项主代理持久化测试，含14个变化组合。不把118项全部称为新独立测试，也不把组合数当作测试函数数。精度工程同样分别报告8项映射及11项结构独立函数，不将导入模块重复执行的测试充数。

R25完整Windows批次的large Markdown Write组，在六类文字精确与duplicate/delete-section两类降级通过后，启动下一测试进程前删除复用reading.json失败（Windows共享占用）。旧进程4528已exit0且stdout/stderr为空；尚无证据确定占用方，不能归为精度失败或宣称产品根因已修复。原失败、8项partial pass和进程日志保留；后两项未执行，完整10项需在逐启动隔离诊断文件的驱动中另行复验。

该复验现已完成：QA提交9390bdd让每次启动独享search/reading/ui诊断路径，20次实际launch完整执行原large MD十类，全部通过，最大未夹取y误差1.018753052px，未改产品、输入步骤或精度预算。结果在 `precision-b-final-r25b-external-large-md-lifecycle-write/result.json`。联合独立审计 `evidence/reading-precision/master/b-r25-windows-final-cohort-independent-audit.json`明确保存原失败及补验引用，核对冻结计划/驱动/二进制、198个未夹取观测、5个夹取、28份旧新文本及独立标注对应关系；原23组不能追改为零失败。

五个夹取分别为large TXT副窗格关闭换行的document-start误差2.506240845px，长单行关闭换行/切Split/回Write各0.949996948px，以及原58240字素document-end误差87.53125px。均单列且不进入未夹取≤2px分母；实际目标仍与视口相交，长单行实际x坐标也留在Point/截图证据中。Windows18组legacy与最终search均通过；正常/异常关闭、编码、原稿字节、选择/焦点、路径/rename/Save As、导航、坏状态/写入/原子失败及语义检查分别保留原始结果。

## R25正式测量、补验与R26边界（2026-10-24）

R25的48份完整App::update原始probe由主代理重新计算，24个格式行均核对样本数、零丢弃、P50/P95/max、同输入协议及预算；审计文件为 `evidence/reading-precision/master/b-r25-full-update-independent-audit.json`。诊断OFF双平台全部通过，Windows诊断ON也全部通过。Linux ON普通Markdown wheel P95从15.986139变为21.764383ms，增量5.778244ms超过3.1972278ms；TXT从16.683884变为20.745884ms，增量4.062ms超过3.3367768ms。这两项是失败，不因正常模式通过而抹去。原始最大值和启动长尾保留，不承诺任意帧率或物理显示时延。

Linux六阶段原始结果在 `artifacts/reading-precision/linux-final-r25-perf/six-stages.json`；热捕获/计划/提交P95最大0.088701ms、单次最大3.133585ms，通过2/8ms预算。索引各规格n=1，不能从一例推断一般分布。完成匹配独立执行n=10/规格，普通/1MiB/10MiB P95约0.983/18.073/177.617ms；同一完成批次旁列MD/TXT不算两批。正常滚动没有match时仍记n=0。分kind布局诊断有2000条容量，达到n=2000时P50/P95/max仅代表保留的末尾窗口，max不称整个会话全部布局调用绝对峰值；完整App::update另有total==samples与零丢弃核验，不受该窗口限制。墙钟、整个update与内存采样范围分别标注；线程CPU未测。

三项补验独立审计见 `b-r25-supplements-independent-audit.json`：自然running enrichment/wheel、match/wheel、match/tree原生输入到退出观测为31.933/38.904/62.136ms，切换后的新小稿62.136ms可用。保留实际任务ID、buffer、revision、自然运行checkpoint和原生输入时间；未人为延长任务。生产GeometryCache实际12个60k字Galley（全部Arc保活）的checkpoint占用1920640B，小于2MiB；第13次索引build证明淘汰，40次重复命中未再build。该上限不包含完整字体、Galley、分配器或总RSS。worker过程VmHWM最大769636KiB；正式50ms采样的app RSS配对增量普通/1MiB/10MiB为4411392/7045120/35659776B，不包含renderer，不称精确分配器峰值。

默认路径迁移在独立原生Linux `/tmp` 的HOME/XDG各目录中，旧A与当前B子进程均未设置 `MDCARD_READING_STATE_PATH`：实际schema2偏移799导入schema3仍799。旧实例继续写v2，v3不变；真实清除后重启不重导，新的实际滚动400.235321可持久化。8份原生默认路径最终文件已明确复制到证据目录并独立核对字节；主代理看过实际导入及清除重启截图。第一次Read控件准备失败单独保留，不当作迁移产品故障，也不删除。

R26只修改 `src/rich_ui.rs` 的诊断文件发布：同一update同步构造完整紧凑JSON、同目录临时文件、保留权限和原子替换均保持，取消这个可丢弃诊断快照的fsync。真实原稿、导出及阅读状态持久化writer不变；阅读定位、几何、主程序与update_probe共7个保护文件rawSHA不变。诊断文件不承诺断电持久。首Windows并发读测试因合法的替换AccessDenied失败，保留日志；测试修正为发布失败仍只能读完整旧/新快照，读者退出后重试必须成功，生产没有重试或计时范围改变。

当前R26冻结清单为 `artifacts/reading-precision/stage-b-r26-source.json`，SHA256 `f15f50c3e87de36ec204221209d5ebd98083ed98629d522b3e84c9acb53d582b`；Windows实际应用 `13d8e38d215f3b4a5bba2a61944a94ce3d7ad1dd93133ab9ff0148a046939187`，Linux `c72763777aec9fb357f6f2800b82b64c79c19b69c35702e591a4316ccfcaf647`。当前新构建检查为Linux209通过/3忽略、Windows native202通过/2忽略，search123/3忽略、perf-harness125通过，fmt和双strict Clippy通过。定位模块仍为685a8475，R25独立1400为相同组件证据，不冒充新GUI或重新执行的语料。

R26短GUI已执行，rich报告12字段完整；三phase实际取消上界33.993/15.986/73.310ms，真实图片/公式同字素误差1/0px、2.1s稳定。正式Linux12与fresh完成匹配均已退出，正常OFF全部通过；ON仍失败：普通MD wheel增量8.754836ms超过3.9228986ms，1MiB TXT all/wheel增量4.708248/8.130158ms超过4.3091526/3.6247706ms。主代理独立重算24份raw及12格式行，见 `b-r26-full-update-independent-audit-linux.json`；这说明取消该诊断fsync不足以解决超预算，不能挑选样本重采。当前继续检查同步诊断构造/序列化成本，Windows新配对和完整包尚未运行。

## R27隔离性能调查（未进入生产）

先用六份实际R26 reading快照恢复typed状态，保留旧report函数作为参考，固定各n200 AB/BA，逐样本比较新旧实际encoded→parsed完整JSON。扩展验证Some/None、Unicode/proof、冻结任务状态、2000条布局、负零、非有限和亚像素，均通过；typed恢复与原snapshot仅对无序records数组做规范化，每次旧/新serializer比较不使用此规范化。原错误oracle的完整stderr及修正过程保留。R2原始构造测试由主代理重算，P95最大收益0.338009ms；R4扩展测试另有完整结果。该收益不足解释R26整段update的8ms以上失败，未据此修改生产。

随后冻结三报告rich→reading→UI真实顺序，以ordinary/1MiB、MD/TXT的实际旧/新最终JSON字节预读一次，在与正式报告相同的/work 9p挂载新目录中，原样旧Document atomic_write和R26诊断helper配对，其他两项均std::fs::write，固定各n200 AB/BA。8行共1600样本、每write及三write直接outer墙钟全部保留；未重采、无错误、最终24文件与输入逐字节一致。三write P95旧→R26：ordinary MD34.887→25.681ms、TXT46.576→38.491ms，1MiB MD42.591→33.546ms、TXT51.302→38.594ms。仅在这轮隔离I/O调查中变快，不能覆盖正式GUI失败或声称已找到根因。

主代理已重算全部样本、AB/BA索引、嵌套timer范围、13份封存文件SHA及24份最终字节；独立审计见 `evidence/reading-precision/master/r27-synchronous-io-independent-audit.json`。构造调查审计为 `r27-serialization-investigation-r2-independent-audit.json`。完整artifact含源码、Cargo.lock、实际ELF、输入/编译清单与全部失败日志。下一步为仅隔离构建添加内存分段计时，保持原整update observer与同步工作范围，退出后写计时文件；它是因果调查，不代替最终构建预算验收。

R27真实update分段调查四run均正常退出，16份进程数据独立审计通过，见 `evidence/reading-precision/master/r27-full-update-phase-independent-audit.json`；审计源码提交 `112e744`。每次调用按序号核对原observer的frame/pass/wheel，0队列丢弃、0phase溢出；退出时原observer共少发布32条尾部，全部非滚轮，完整phase尾部仍保留。队列丢弃0不能冒充全进程所有调用均已发布，最终测量须明确这一采样范围。

在本轮调查的wheel调用中，reading同步写入P95旧→候选依次为：普通MD3.180427→7.387832ms、普通TXT3.380261→8.018245ms、1MiB MD3.264121→8.424431ms、1MiB TXT2.567888→5.881853ms。候选reading构造P95约0.199–0.247ms，其余update的P95约0.609–0.687ms。各段P95不能相加当作整调用P95；原始实际慢call逐段数据完整保留。本轮普通MD/TXT整调用wheel P95仍变慢，1MiB两格式本轮较快，不能拿该仪器版替换R26失败样本。

下一步只在artifact中测试固定两个diagnostic sink复用文件句柄的方案，JSON字段、内容、每update频率和同步范围不变；要求实际文件身份、readonly、删除/替换/换path/写入失败检测及有限重开，文件长度缩短无尾字节，原rich原子快照与真实reading-state durable writer不改。原50ms并发reader保留，身份检查本身也计时。测试及同挂载固定n200配对有证据前，不进入生产。

R27句柄方案v2的固定n200 AB/BA调查已通过9类安全断言：Unicode及缩短、权限保留、readonly失败清空/恢复、删除重建与原子替换的旧inode字节不变、换path、symlink目标保护、真实FIFO及时拒绝、缺父目录后恢复及固定两handle。两次真实dev+ino/readonly校验全计入wall；原50ms reader函数保留，但这只是一个artifact进程标为a/b、两报告读写，不冒充双GUI进程内存测量。

相同R26输入buffer、同rich helper下，普通MD/TXT和1MiB MD/TXT reading写入P95的stdwrite→held分别为9.698246→3.930442、10.927585→1.548520、9.560566→2.901329、9.101077→1.569166ms；三write外层总P95分别23.850659→15.962096、20.463733→8.649046、18.000691→13.767978、16.718328→10.691386ms。最大值及所有原始样本保留，不用P95掩盖自然I/O长尾。

主代理已独立复核1600样本、16份封存文件、12个唯一最终文件字节、每组2opens/398reuses/400次prepost及两并发reader结果，见 `evidence/reading-precision/master/r27-held-sink-independent-audit.json`。原v1及补O_NOFOLLOW|O_NONBLOCK和truncate(false)前的版本保留。证据支持R28最小生产候选：仅Linux reading/UI诊断两固定sink，Windows仍stdwrite；所有JSON构造/字段/每update/同步位置、rich原子快照、真实reading-state writer和原update_probe均不变。尚须重建全部产物及正式双平台预算，不提前记通过。

## R28正式记录完整性故障（保留原件）

R28b实际构建清单SHA256为 `d8409715af04fc24de70ec1bff63b31e5a8a6905cbd246601423e24a10e57e20`，82列入/70实际/5份dep-info/三bin已独立审计，见 `b-r28-build-inputs-independent-audit.json`。Linux212通过/3忽略、Windows native203通过/2忽略、search123/3忽略、perf125及fmt/双strictClippy通过；新短GUI三phase实际取消68.445/49.752/78.869ms，PNG/公式同字素≤2px且2.1s稳定，完整诊断字段未剪。

正式Linux12run与fresh completed虽均exit0，主代理实际解析发现 `evidence/reading-precision/implementation/linux-final-r28-perf-off-candidate-ordinary/b-full-update.json` 最终为0B。派生失败日志及空文件原样保留，不能称OFF或全预算通过，也不能仅据run exit0补写/宣布通过。其余原始文件仍保留，缺失raw不能由汇总分位数反推重建。

原cf687观察器的后台线程使用std::fs::write覆盖，并有100ms睡眠；全局Sender不在正常关闭前显式drain/flush。空文件与覆盖后尚未完整写入的退出窗口相容，但没有syscall trace，具体发生点不当作已证根因。下一方案为观察器自身原子快照和正常退出的有限flush/ack，旧264基线与当前候选使用同一新观察器；保留原first-statement至final-guard的计时范围，原产品同步诊断工作仍在范围内。方案与测试尚待审查，不以旧完整前缀替代缺失原始样本。

R29观察器源码独立边界审计已通过，见 `r29-sampler-boundary-independent-audit.json`：相对R28只改main/update_probe，first App::update statement、start/tag函数体（去空白）和Drop首elapsed表达式均与旧观察器一致。当前冻结observer SHA为 `745f2725783243ee8359b58856c67f2d4f89a2c90e2605867f6edbaf85fae699`。fmt独立检查已执行exit0。

当前R29实际full core日志为219通过/3忽略，Windows本机native为210通过/2忽略（native SHA `5555163c48468d8e900ab66a579f4a5c8d8f7bb998949175adc0d75786ca9de1`），源数据HOME/XDG/AppData/cwd/temp均按子进程隔离。主代理已直接读取stdout与run.json，尚待完整source/dep-info/bin清单绑定。忽略的测试名称保留：`search::tests::followup_search_timing_across_full_verification_and_cpu_contention`、`ui_polish_tests::headless_scale_language_theme_mode_matrix`（两平台），以及Linux的 `export_ui::process_tests::followup_cancellation_handoff_records_live_vs_terminal_descendants`。这些额外CPU竞争/432无窗口布局项目未在本轮执行，不能与通过数量相加或替代原生GUI。

## R29当前绑定与正式性能结论

产品提交 `40b5e3bb16e1658e333cf2fddf965a22d4e878ff`；Linux驱动QA提交 `ed2c00eb58eb7412dd0da5821d279da272b388ed`，Windows驱动QA `f89fb24`。当前清单SHA256 `655cfdafcdd4feae4d3199206b49a39ba72c497966adec6378bcfe32d2210b55`。Windows应用 `face40f4af716c004b39e4ce992af088dedda171f924a73df8cf8504eefb67e1`，Linux `b438bd9e042366a77c63bcb9f6ccc989414119788959086cec98114f2c6cd9cf`，Windows native `5555163c48468d8e900ab66a579f4a5c8d8f7bb998949175adc0d75786ca9de1`。82个冻结输入逐一绑定Git；locales/ui.json与src/text_files.rs的CRLF/LF差异明确记录，JSON语义亦核对，原始编译SHA保留，不宣称逐位可复现构建。见 `b-r29-commit-source-binding.json` 与 `b-r29-build-inputs-independent-audit.json`。

旧264功能基线只加入同版745f2725观察器与正常退出刷新钩子，86冻结文件/84原Git文件和两dep-info已独立复核；main移除明确观测块后逐字等原264。Windows基线 `ef2cd21a784d97aee9a04fc9528e2258898cb7ef0d3abad505ce19dbde39299f`，Linux基线 `57b810455c5b568a1f97b9704b472a753f30fef49bb88642bad95e88cb3c9e2e`，见 `b-r29-rebuilt-baseline-independent-audit.json`。没有把新定位或缓存功能回填基线。

联合独立审计 `b-r29-full-update-independent-audit.json` 验证48份实际最终raw：全部flush_complete，submitted=processed=total=len(samples)，queue/retention丢弃均0。两平台24个格式行，OFF/ON各自all/wheel P95增量均满足原max(基线20%,2ms)，全部长尾和最大值保留。Windows为nearest-rank，Linux为floor(n*p)，仅在各平台原配对内比较。正常OFF候选wheel P95：Windows普通MD/TXT0.5283/0.4585ms、1MiB0.5455/0.3108ms、10MiB0.4515/0.3434ms；Linux0.4928/0.3525、0.4661/0.3847、0.4940/0.3250ms。诊断ON另有完整数值表，不用正常模式覆盖它。

六阶段审计 `b-r29-six-stages-independent-audit.json` 从原始数组重算；hot六行P95最大0.090382ms，最大单次1.969573ms，通过2/8ms预算。每规格source-index只有n=1；布局n=2000是保留尾窗，不是全程绝对峰值。完成match总计30次，单个n10批次旁列MD/TXT不加倍，正常滚动match n=0仍为0。独立prefix实际字节对应亦核对。批量Rust原摘要使用lower-median/nearest95，统一六阶段派生表用floor(n*p)，各自公式明确核对，未混作同一统计。全部为函数墙钟，线程CPU未测，非显示延迟或FPS。实际bench ELF0c483ea4已单独保留，Cargo.lock、Rust驱动、builder与pure685a分别绑定；R29是新执行样本。

R29实际running三phase取消56.119/52.702/74.829ms，真实PNG/KaTeX同字素及2.1s稳定，完整诊断schema未剪；正常close smoke基线72/72、候选73/73最终计数/flush/drop均正确。首QA错误路径r29b的FileNotFound发生于Popen前，旧失败与新freshstage均保留，不当产品崩溃。全部旧R25/R26预算失败、R28空raw及R27调查未删除或重写。

当前完整包及解压包关键GUI仍待执行；审定计划 `artifacts/reading-precision/package-plan-r29-final-master.json` 保留原6组11Linux和8Windows、所有原始失败目标、资源归属/原稿SHA/字素与2px/夹取/2s稳定规则。包sourceCommit可为后续文档快照，binarySourceCommit必须为上述实际产品提交；最终验收会作为不可变包外的独立记录，避免把未执行包测试提前写进包。
