# R29正式发布与资源清理（2026-10-24）

> 本文记录 R29 发布当时的清理快照。当前 Windows 已为 Icon Suite R3，Linux 仍为 R29；本日后续图标候选包与缓存清理见[图标交付后清理](RESOURCE_CLEANUP_ICON_SUITE_2026-10-24.md)。下文“当前”均指该次 R29 发布时点。

R29现为本地最新正式版（stable），`dist/latest.json`、`dist/windows/MDcard.exe`、`dist/linux/mdcard`一致指向build `precision-b-20261024-r29`。产品提交40b5e3bb、包源码快照8c09f82、独立验收提交0bea1e9分别保留。程序/完整包/内嵌manifest字节不变，没有重新编译或重打包；内嵌acceptanceCandidate是构建时历史标识，正式状态由外部latest和promotion.json（历史本地证据：`../evidence/release-r29-2026-10-24/promotion.json`，未公开）记录。

Windows22059、Linux22003个payload在迁移前和正式目录均全SHA/长度/精确集合通过。正式目录下实际Windows普通MD/大TXT重排18项通过；Linux真实图片/公式异步加载后锚点分别1px/0px，稳定观察2.1秒通过。此处是发布路径复验，不宣称重新运行全部38项或重新测量性能；原验收范围与限制见[交付记录](READING_PRECISION_DELIVERY.md)。

## 实际清理

工作区删除 **227,564个文件，60,295,884,087字节（56.15GiB）**，另删除26个链接本体。Docker本项目编译卷删除 **17,096个文件，10,819,391,118字节（10.08GiB）**。合计 **244,660个文件、71,115,275,205字节（66.23GiB）**。

D盘执行窗口可用空间由190.30GiB变为244.63GiB，净变动54.33GiB；该读数可能受其他进程影响。以上删除量是文件逻辑字节，不等于物理净回收量；Docker卷清空不保证虚拟磁盘立即缩小。

逐路径计划见cleanup-plan-final.json（历史本地证据：`../evidence/release-r29-2026-10-24/cleanup-plan-final.json`，未公开），每个执行结果及理由见deletion-results.jsonl（历史本地证据：`../evidence/release-r29-2026-10-24/deletion-results.jsonl`，未公开），统计见cleanup-result.json（历史本地证据：`../evidence/release-r29-2026-10-24/cleanup-result.json`，未公开）。19处Cargo target、旧工作台/首阶段运行目录与旧完整包、候选包/重复解压、旧应用/测试程序、Python缓存均按显式路径处理。当前源码、Git、正式运行目录、当前完整包及原稿/用户设置不在删除范围。

| 删除类别 | 计划目标数 | 文件数 | 逻辑字节 |
| --- | ---: | ---: | ---: |
| `archived_link_metadata` | 26 | 0 | 0 |
| `build_cache` | 19 | 36236 | 17,886,164,645 |
| `obsolete_binary` | 51 | 51 | 4,526,746,184 |
| `superseded_archive` | 4 | 4 | 2,430,127,893 |
| `superseded_output` | 5 | 154264 | 15,652,365,486 |
| `verified_archive_copy` | 37009 | 37009 | 19,800,479,879 |

原始reading-state/reading-precision展开证据只在同路径、同大小、同SHA256已存在于完整证据ZIP（历史本地证据：`../artifacts/reading-precision/delivery/MDcard-reading-precision-evidence-20261024-r29.zip`，未公开）时去重。归档含37401个输入文件，SHA256 `a6a8b848ca85c78eba77f269de11bea8e1de78196dde5c12635c6cee8cbdd3fc`，包括失败/降级/超时及原始输入、截图、性能raw、测试ELF。归档后变化和未覆盖文件保留；没有挑选删除失败案例。旧报告里已移除的展开路径是采集时路径，可按ZIP同名成员提取；不把旧路径改指新程序。被替代且不在证据包中的旧完整运行包可按历史源码重新构建，但不承诺位级相同。

## 没清理的全部项目与理由

逐文件、逐目录、链接元数据和字节大小全部列在retained-files.tsv（历史本地证据：`../evidence/release-r29-2026-10-24/retained-files.tsv`，未公开），分类统计见retained-summary.json（历史本地证据：`../evidence/release-r29-2026-10-24/retained-summary.json`，未公开）。包含.git内部文件；不跟随任何链接。清单自身统计大小留空以避免自引用，内容反映采集时状态。

| 保留类别 | 原因 |
| --- | --- |
| `.git/`、`.github/` | 完整历史/回溯和CI，不改写历史或全局GC |
| `src/`、Cargo文件、`vendor/` | 产品/本地依赖源码和版本锁；vendor不是可删缓存 |
| `assets/`、`locales/`、许可证 | 当前字体、字体源、语言和分发合规材料 |
| 新增`assets/branding/` | 清理期间出现的独立图标资源工作，未纳入已验证删除计划，保持原件且不代为提交 |
| `renderer/` | 本地引擎源码、锁文件及运行时来源；完整运行依赖位于dist |
| `scripts/`、`tests/`、`examples/` | 可重建/回归源码、精确编码夹具和使用示例；只清理生成缓存 |
| `dist/windows`、`dist/linux` | 唯一当前可直接运行的完整R29，Node/Chromium/Java/Pandoc/字体均必需 |
| `dist/MDcard-*-precision-b-20261024-r29-*`、`dist/latest.json` | 正式可分发包和稳定身份入口，与展开运行目录用途不同 |
| `artifacts/reading-precision/delivery/` | 原始证据ZIP、逐成员清单、闭合哈希及校验过程；不能由源码重建历史观测 |
| 源码ZIP/补丁、R29构建清单及b-r29摘要 | 小型直接取证入口；避免必须解压整个证据包 |
| 本轮release-r29实际GUI输入和证据 | 旧ZIP生成之后的新证明，不能拿旧归档替代 |
| 其他模块evidence/实际导出/夹具 | 未证实属于本次归档，不冒险删除唯一历史输出；全部逐文件列理由 |
| 配置、README及下表每份docs | 当前操作/构建配置，或仍有独立合同、失败/限制和历史身份价值 |

### 每份文档

现有文档逐份审查后没有可证明纯重复、无用途的文件，所以未按日期删除；已修正README、验收总索引、正式交付和旧清理报告中的过时“当前”措辞。归档中可精确取回的一次性实现工作报告及中间Markdown随展开副本清理。

| 文档 | 保留原因 |
| --- | --- |
| `ACCEPTANCE.md` | 历史模块验收总索引；当前正式入口已更新为R29 |
| `BUG_AUDIT_2026-10-22.md` | 已修缺陷的复现、根因、原始失败及未验证边界 |
| `BUG_AUDIT_FOLLOWUP_2026-10-22.md` | 搜索/导出后续修复与性能失败历史 |
| `GLOBAL_SEARCH.md` | 搜索、快速切换、替换及回滚操作合同 |
| `LARGE_DOCUMENT_PERFORMANCE.md` | 虚拟化架构、内存预算和性能测量边界 |
| `LOSSLESS_EXPORT.md` | 导出、取消、离线资源及发布失败保护合同 |
| `MATH_DIAGRAMS.md` | 公式图表语法、源码同步和本地资源安全边界 |
| `MATURITY_ROADMAP.md` | 未实现功能、优先级与后续验收门槛，避免混同已完成范围 |
| `READING_PRECISION.md` | 文字/逻辑行精度、冻结预算及分级降级合同 |
| `READING_PRECISION_ACCEPTANCE.md` | 完整原38项验收与未测/忽略项边界 |
| `READING_PRECISION_DELIVERY.md` | R29交付入口、完整包哈希和两阶段结论 |
| `READING_PRECISION_IMPLEMENTATION_A.md` | A阶段实现侧证据，职责与独立验收不同 |
| `READING_PRECISION_IMPLEMENTATION_B.md` | R29实现、诊断改进及历史失败调查 |
| `READING_PRECISION_INDEPENDENT_A.md` | A阶段独立验收及旧包范围勘误 |
| `READING_PRECISION_INDEPENDENT_B.md` | 独立评测、真实GUI、失败复验与性能结论 |
| `READING_PRECISION_STAGE_A_PACKAGE.md` | A独立提交、旧完整包身份及限制溯源 |
| `READING_STATE.md` | 文档身份、当前位置/最远位置/已读和持久化合同 |
| `READING_STATE_ACCEPTANCE.md` | 首阶段历史验收，不由后续精度结论覆盖 |
| `RESOURCE_CLEANUP_2026-10-22.md` | 上次实际清理、旧路径去向及历史保留清单 |
| `RESOURCE_CLEANUP_2026-10-24.md` | 本次正式发布、实际删除与全部保留理由 |
| `STRUCTURAL_EDITING.md` | 表格/块操作、源码同步和原子撤销合同 |
| `UI_POLISH_2026-10-22.md` | 既有视觉与交互设计依据 |
| `UI_POLISH_ACCEPTANCE_2026-10-22.md` | UI独立验收与真实平台限制 |
| `WORKBENCH_ACCEPTANCE_2026-10-22.md` | 工作台验收、真实失败及DPI范围 |
| `WORKBENCH_REDESIGN_2026-10-22.md` | 工作台布局、组件及交互设计合同 |
| `WORKSPACE_ROOT_FIX_2026-10-22.md` | 工作区根目录行为、根因及专门回归 |
| `releases/0.1.1.json` | 历史版本/标签、源码、包散列与证据索引 |
| `releases/2026-10-22-bug-audit-followup.json` | 历史版本/标签、源码、包散列与证据索引 |
| `releases/2026-10-22-bug-audit.json` | 历史版本/标签、源码、包散列与证据索引 |
| `releases/2026-10-24-r29.json` | 正式R29发布身份 |

### 工作区外与开发环境

仅清空经inspect核对的`mdcard-dev`挂载卷`mdcard-target`的`/work/target`内容；卷本身保留。`mdcard-cargo`依赖下载卷、mdcard-dev容器、基础镜像保留，因为它们提供可复用开发环境，不是本轮独占的无用产物；未执行Docker全局prune。系统临时目录、其他项目、真实应用设置/稿件/恢复数据不在本次工作区清理范围，不做递归扫描或删除。

清理后当前双平台payload及包SHA再次核对，结果入口为post-cleanup-verification.json（历史本地证据：`../evidence/release-r29-2026-10-24/post-cleanup-verification.json`，未公开）。本报告与完整保留清单由实际执行结果生成，未将估算参数作为实测结论。
