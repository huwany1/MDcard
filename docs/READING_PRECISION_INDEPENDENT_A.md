# 阶段 A 独立验收快照（2026-10-23）

> 后续范围勘误：R17真实验收发现普通副窗格源码视图缺少文字几何，R19又发现large TXT首次历史挂载的同布局raw偏移误差4.756226px。上述旧包不包含这两项修复，不能据此宣称所有窗格精确恢复完成。后续修复及当前复验见 `READING_PRECISION_INDEPENDENT_B.md` 和38项验收表；本快照的历史数据及包哈希保持原样。下文“当前”仅指R10历史快照；现行阶段A支持范围须结合READING_PRECISION_INDEPENDENT_B.md及完整38项验收表。

现行阶段A已由R29修复后的实际完整包复验通过，阶段B亦单独满足独立评测与GUI门槛，见 [最终交付](READING_PRECISION_DELIVERY.md)。以下R10旧数据和旧包结论保留为历史快照，不能代替当前支持范围。

阶段 A R10 在以下已测文字路径通过字素身份与实际几何验收，Windows 25 组运行共 227 项 GUI 断言通过。阶段 B 尚未完成；整个 GUI update 预算与后台索引真正停止计算仍未满足全项验收。因此本快照交付已验证的阶段 A 功能和包，不宣布整个目标完成。完整编号与保留项见 `READING_PRECISION_ACCEPTANCE.md`，实现侧 Linux/性能证据见 `READING_PRECISION_IMPLEMENTATION_A.md`。

## 当前源码和构建

- 产品清单：`artifacts/reading-precision/stage-a-r10-source.json`，28 个当前产品路径逐一核验，摘要 `5c0096fa3c1de55f9d55e972aa0b4ed353562659ee2dbcdd7f82412263461f75`。阶段 A 提交包含本快照及该产品版本；包 manifest 另记录实际 sourceCommit/binarySourceCommit，不把初始基准 HEAD 当成实现提交。
- Windows 应用 SHA256：`3dd8edc357f79edf28ffa377c9baa657f5f7040bc90e32b530c485a15b414374`。
- Linux 应用 SHA256：`cfa218d0eafd5748b16ae55417fa6eb2ae2df285257ded11e50c0a422864e148`。
- 当前独立库存：`evidence/reading-precision/master/review-a-r10-20261023.json`。它保留各代成功和失败、每个 result 的 SHA 与当前二进制关联；不是仅凭库存计数判定通过。

平台为 Windows 11 x64 build 26100、125% DPI，原生 HWND 键鼠输入和实际窗口截图；Linux 为 Debian 12 / X11 / Xvfb / Openbox / Mesa 软件 GL。没有据此验证其他 DPI、GPU、Wayland、其他 Linux 桌面或 macOS。

## 独立测量方法和结果

主代理脚本不向应用注入位置、恢复结果或延迟。通过真实输入取得阅读位置，读取当前绘制 Galley 的实际矩形；另从原稿独立解码，核对 SHA、原 UTF-8 范围、完整字素、逻辑行和视口矩形差。Windows .NET StringInfo 与独立 Rust 字素期望共同校验边界。旧保存偏移、候选分数和应用 exact 标志均不足以判对。主代理另核看当前大 Markdown 字号前后、巨长 TXT 后部、large mapping 和 Linux 异步加载截图。

| 当前运行（`evidence/reading-precision/master/` 下） | 断言数 | 覆盖 |
| --- | ---: | --- |
| `a-r10-full-reflow-read` | 36 | 普通/大 MD/TXT，各四次宽窄、侧栏、字号及 Split 往返 |
| `a-r10-full-reflow-write` | 52 | 同四类，另含换行开关和分屏/源码往返 |
| `a-r10-full-lifecycle-read` / `write` | 20 / 20 | A→B→A、正常重启、真实异常 kill 后成功落盘记录 |
| `a-r10-utf16-reflow-read` / `a-r10-gb18030-reflow-write` | 9 / 13 | UTF-16 CRLF 与 GB18030 解码坐标、字素与重排 |
| `a-r10-mega-middle-reflow-read` / `a-r10-mega-late-reflow-write` | 9 / 13 | 约1.9 MB单段中部与单逻辑行92.77%后部、卸载重建 |
| `a-r10-ordinary-mapping-read` / `a-r10-large-mapping-read` | 1 / 1 | 独立十类源范围，生成节点前后与实际复杂字素；475200字节路径单独覆盖 |
| `a-r10-final-boundaries-read` | 4 | 空稿、真实3200px图片中部往返及重启、文末夹取 |

以上 11 组为 178 项精度专项断言。另在 `evidence/reading-state/master/precision-a-r10-*` 下完成当前构建的 routes、guards、source、secondary、same-file、rename、failure、atomic、corrupt、clear、capacity、priorities、semantics 和 core，共 14 组 49 项既有能力回归。两部分合计 227 项；断言数不是独立文档数、唯一字素数或阶段 B 算法样本数。

137 个未夹取文字恢复样本最大纵向误差 **0.7375030517578125 logical px**，每次验证同一个原始完整字素和至少 2.1 秒的稳定尾段。同布局切换/重启验证原始 x/y 偏移，两轴均在 2 px 内。文末夹取实测 **87.625 px**；巨长单行关闭换行后的三个 y 范围夹取也单列，仅验证同字素可见、原因、实际误差和稳定，不计入 ≤2 px 分母。

Read 四类异常 kill 时，最后保存的文档记录年龄分别 **1946、2030、2174、2203 ms**；年龄和最近一次成功事务时间分别保留，不承诺零丢失。光标/选区可保持文首，而阅读视口恢复到后部；主副窗格和同 ID 同 revision 不同 buffer 的状态分别验证。未命名草稿仅验证独立身份，未宣称草稿正文跨重启恢复。

源码到预览验证覆盖普通文本、强调、链接、转义、行内代码、列表、引用、表格、围栏代码和生成节点前后。实体以原稿代表字素和完整 token 分列；生成节点没有被当作原稿文字。实际观察了普通 emoji、组合音标、肤色及 ZWJ 肤色序列，而非只验证 ASCII 对照。

## 自动化、故障和资源边界

当前 R10 独立 Rust 字素/实际 Galley 测试 **8/8**，日志 `artifacts/reading-precision/master/independent-r10-source-tests.log`；旧状态独立合同 **11/11**，位于 `independent-r10-state-tests.log` 的 73 项总测试中，其余为导入模块测试，不能相加冒称独立数量。当前 Linux 根测试 153/3 ignored、Windows 原生 146/2 ignored、fmt 和两平台 strict clippy，以及 6 项新增故障测试分别有独立日志。

GUI 真实覆盖五种打开入口、保存失败与取消打开保护、相对/绝对路径、不同目录同名文件隔离、另存为继承及原记录保留、内部重命名、非零横向源码滚动、主副窗格、同 ID 同 revision 的 buffer 隔离、真实用户导航优先、last position 不被 furthest 覆盖和到底不自动已读。写入失败、真实 Windows FileShare 阻止原子替换后旧记录保持、重试、五类坏状态文件、单个/全部清除与512记录淘汰均通过；所有精度夹具原稿字节保持。

当前 Linux 异步 PNG / KaTeX 的同字素最大位移为 1 / 0 logical px，总补偿 29 / 2，最终 2.1 秒无新增补偿。当前待布局任务的原生滚轮取消上界 50.305 ms，不能把它当作后台索引计算已中止。

三档 53 KiB / 1 MiB / 10 MiB 同输入的热路径 P95 分别 0.059445 / 0.055393 / 0.055997 ms，最大值 2.037626 / 0.327455 / 0.085369 ms，达到冻结的 2/8 ms 预算。它不包含真正字体布局和后台磁盘事务；GUI 旧 main-section 字段排除了入口前置和尾部处理，**不能用于证明完整 update 预算**。P50/P95/max/n、后台索引、实际排版、写盘、RSS、写入频率和观测限制均在实现报告及原始采样中保留，不称线程 CPU、物理显示延迟或 FPS。

## 保留的失败和未完成工作

历史产品失败（亚像素循环、实体/围栏源映射、Windows 图片 URI、资源尚未布局覆盖历史、虚拟窗口丢 pin、字体重排的失效 target）及输入驱动失败（点击过短、滚轮落入公式画布、滚轮数值超范围、原生边界定位假设）均保留；当前成功不删历史记录。

阶段 B 仍需完成真正可取消的索引/匹配、同观测的整个 update 对旧 `264d42d` 基线、1400 个冻结独立样本达到98%/观测零错误精确、内外编辑真实 GUI、任务取消/清除后不复活和最终包复验。阶段 A 的纯定位器对变更稿仍保守降级，1400 分母中的精确命中为0，不是阶段 B 通过。图片、公式、图表、隐藏语法以及不能映射的节点仍明确降到块级，不能宣传任意内容文字精确。
