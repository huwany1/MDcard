# 资源清理与完整保留说明（2026-10-22）

> 历史清理快照：下文“当前”指2026-10-22工作台版。现行正式版已为R29，最新运行入口、实际清理结果和逐文件保留理由见 [2026-10-24清理记录](RESOURCE_CLEANUP_2026-10-24.md)。

本轮清理以 `dist/latest.json` 指向的 `workbench-final-20261022` 为唯一当前交付。删除旧程序、重复完整包、编译缓存及空目录；源码、完整 Git 历史、最新完整运行资源和原始验收证据保留。

旧版同名清理记录已被本记录替代，历史内容可通过 Git 恢复。没有把仍有缺陷分析、操作契约或验收边界的文档仅因日期较早而删除。所有保留文档及理由逐项列于下表。

## 实际清理结果

- 工作区删除 **269,365 个文件，32.45 GiB**，对应 24 个计划目标，另移除空的根 `target/`。
- Docker 本项目编译缓存删除 **15,253 个文件，9.98 GiB**；卷仍存在且内容已为空。
- 两处合计删除 **284,618 个文件，42.43 GiB** 逻辑内容。
- D 盘可用空间从 221.66 GiB 增至 254.26 GiB，执行窗口内净增加约 **32.60 GiB**；这是磁盘读数，可能受其他进程活动影响。

删除前的全部绝对目标、文件数和字节数见 plan.json（历史本地证据：`../evidence/resource-cleanup-2026-10-22/plan.json`，未公开）；逐项目执行结果和 Docker 项目缓存统计见 result.json（历史本地证据：`../evidence/resource-cleanup-2026-10-22/result.json`，未公开）。统计为文件逻辑字节，不能直接等同磁盘净回收量；Docker 虚拟磁盘不会因内部删除必然立即缩小。

| 删除项 | 原因 |
|---|---|
| `artifacts/workbench-redesign-2026-10-22/r1`–`r9` | 开发阶段程序；最终相同程序已在 `dist`，中间状态可由 Git 源码重建 |
| `artifacts/workbench-redesign-2026-10-22/packages/` | R6/R7/R8/R9及最终快照的完整包和展开目录；最终副本已逐文件核验存在于 `dist` |
| `artifacts/ui-polish-2026-10-22/packages/`、`bin/` | 已被工作台最终版替代的 R11 完整包和独立测试程序 |
| `artifacts/workspace-root-2026-10-22/` | 已包含于新版本的工作区修复候选与打包中间资源 |
| `dist/MDcard-0.1.1-ui-polish-r11-20261022-*`、`dist/MDcard-0.1.1-workspace-root-20261022-windows-x64.zip` | 三份过时归档；保留旧验收记录，不保留重复运行时 |
| `tests/{performance,rich,search}-acceptance/target/` | 三套独立验收的 Rust 编译缓存，能由各自 Cargo 锁文件重建 |
| `scripts/__pycache__/` | Python 自动生成缓存 |
| Docker `mdcard-target` 卷的 `/work/target` 内容 | 已核实该卷挂载属于本仓库，且没有编译进程；只清空项目编译缓存，不删除卷 |
| 空的根目录 `target/`、`artifacts/performance/`、`artifacts/search/`、`evidence/search/`、`evidence/ui-gpu/` | 无文件内容，属于旧验证残留 |

## 所有保留类别及原因

下面覆盖工作区内全部根文件和目录。需要精确到每个文件时，打开 完整保留清单 TSV（历史本地证据：`../evidence/resource-cleanup-2026-10-22/retained-files.tsv`，未公开）：包含所有保留文件、目录、逻辑大小和保留原因，连同 `.git` 内部文件一起列出。清单和汇总自身的大小留空以避免自引用；其余大小为采集时值。分类大小见 retained-summary.json（历史本地证据：`../evidence/resource-cleanup-2026-10-22/retained-summary.json`，未公开）。

| 保留项 | 原因 |
|---|---|
| `.git/` | 完整历史、分支、索引和恢复旧文档的依据；不做历史重写或激进 GC |
| `.github/` | 后续 CI 构建和验证配置 |
| `src/`、`Cargo.toml`、`Cargo.lock` | 产品源码、依赖声明及可重复构建的版本锁 |
| `vendor/` | Cargo 实际引用的本地依赖源码，不是下载缓存 |
| `assets/` | 运行字体、生成字体子集的原始字体、来源及许可证；当前程序和生成流程需要 |
| `locales/` | 六语言界面文本和翻译来源 |
| `renderer/` | 本地渲染/导出引擎源码、Node 锁文件、运行时来源和许可证；开发依赖及下载目录本轮开始时已不存在 |
| `scripts/` | 构建、打包、生成字体/夹具和跨平台回归脚本；历史专用脚本仍供复现对应版本，不是当前包已重新验证的声明 |
| `tests/` | 独立验收项目源码、Cargo 锁文件及精确编码字节夹具；仅删除其 `target` |
| `examples/` | 随包功能示例及本地图片，仍用于演示/验证 |
| `docs/` | 逐文档理由见下表 |
| `dist/windows/`、`dist/linux/` | 可直接运行的最新双平台程序和必要 Node、Chromium、Java、Pandoc、字体与引擎。不得只留下主程序，否则离线渲染/导出失效 |
| `dist/MDcard-0.1.1-workbench-final-20261022-windows-x64.zip`、对应 Linux `.tar.gz` | 最新可分发完整包；与展开目录用途不同，两者均保留 |
| `dist/latest.json` | 当前包、程序 SHA256 和源码身份的唯一入口索引 |
| `artifacts/ui-polish-2026-10-22/master/exports/` | 两平台实际九格式导出产物和过程记录，保留供检查历史产物内容 |
| `artifacts/ui-polish-2026-10-22/visual-fixture/`、`artifacts/workbench-redesign-2026-10-22/visual-fixture/` | 真实界面复现使用的多层目录、长路径和示例文稿夹具，体积小且仍被脚本引用 |
| `evidence/ui-polish-2026-10-22/` | 历史原始截图、失败记录、性能样本及上次清理证据；删除旧程序不应抹去原始观测 |
| `evidence/workspace-root-2026-10-22/` | 根目录回归的失败/成功、原生窗口和状态证据 |
| `evidence/workbench-redesign-2026-10-22/` | 最新交付的原始验收、实图、严格滚动失败样本、性能数据和 `review.html`；全部保留，未挑选删除失败记录 |
| `evidence/resource-cleanup-2026-10-22/` | 本次删除计划、执行结果、保留清单、校验结果及生成这些记录的脚本 |
| `README.md` | 使用、开发和最新交付入口；已去掉“中间程序与旧归档保留”的过时说明 |
| `LICENSE`、`THIRD_PARTY_NOTICES.txt` | 项目及第三方许可，分发所需 |
| `Dockerfile`、`.dockerignore` | 可重建 Linux 开发环境及控制构建上下文 |
| `.gitignore`、`.gitattributes` | 生成物排除规则和文本/二进制 Git 属性 |

工作区外的系统临时目录、应用用户稿件/设置、其他项目及全局缓存不在本次清理范围。Docker `mdcard-dev` 容器、`mdcard-target` 空卷、`mdcard-cargo` 依赖下载卷和基础镜像保留，原因是它们提供现有开发环境或可复用依赖；没有执行全局 Docker prune。清空缓存后首次编译会重新生成产物。

## 逐份保留文档的理由

| 文档 | 保留理由 |
|---|---|
| `ACCEPTANCE.md` | 历史模块验收总索引；当前发行入口已纠正 |
| `BUG_AUDIT_2026-10-22.md` | 缺陷复现、根因、修复和回归边界，供防止复发 |
| `BUG_AUDIT_FOLLOWUP_2026-10-22.md` | 并发搜索、导出取消与进程回收的后续修复和失败语义 |
| `GLOBAL_SEARCH.md` | 当前检索、正则替换、快照和回滚的操作/架构契约 |
| `LARGE_DOCUMENT_PERFORMANCE.md` | 当前大文档增量架构、内存预算和性能计时边界 |
| `LOSSLESS_EXPORT.md` | 当前导出使用、配置、格式/离线资源和取消限制 |
| `MATH_DIAGRAMS.md` | 当前公式/图表支持、安全规则与引擎重建方式 |
| `MATURITY_ROADMAP.md` | 已实现与待实现边界、后续优先级和验收门槛 |
| `RESOURCE_CLEANUP_2026-10-22.md` | 本次清理结果、保留理由和证据入口；替代旧清理说明 |
| `STRUCTURAL_EDITING.md` | 当前表格/块操作、快捷键、无损同步与撤销约束 |
| `UI_POLISH_2026-10-22.md` | 历史 UI 设计理由和前后比较，解释工作台改造的来源 |
| `UI_POLISH_ACCEPTANCE_2026-10-22.md` | 历史 R11 的程序身份、验收范围与失败限制，不能冒充当前版本证据 |
| `WORKBENCH_ACCEPTANCE_2026-10-22.md` | 最新工作台验收矩阵、包身份、真实平台与性能限制 |
| `WORKBENCH_REDESIGN_2026-10-22.md` | 最新布局/组件规范、入口取舍与迭代修订原因 |
| `WORKSPACE_ROOT_FIX_2026-10-22.md` | 根目录异常的根因、定向修复和实际回归证据 |
| `releases/0.1.1.json` | 历史版本身份及源文件/产物散列索引 |
| `releases/2026-10-22-bug-audit.json` | 历史审计源码、归档与证据身份，不作为当前入口 |
| `releases/2026-10-22-bug-audit-followup.json` | 历史后续修复候选的身份和证据索引 |

旧报告内的旧路径是采集时记录，部分资源已被本次或之前的清理删除；不保证仍可直接打开，也不把路径改指新程序以免混淆身份。历史源码和已跟踪文档可从 Git 恢复；已删除的忽略文件不在 Git 里，编译/包需按相应历史提交重建，不能保证字节完全相同。

## 清理验证及当前入口

清理前后分别核对 Windows **22,050**、Linux **21,994** 个 manifest 清单文件（合计 **44,044**），检查成员集合、长度、逐文件 SHA256、程序身份与两份归档 SHA256，均通过；前后结果一致。记录见 verification-before.json（历史本地证据：`../evidence/resource-cleanup-2026-10-22/verification-before.json`，未公开） 与 verification-after.json（历史本地证据：`../evidence/resource-cleanup-2026-10-22/verification-after.json`，未公开）。所有计划删除路径均已不存在，Docker 项目编译卷已为空；`git diff --check` 通过，已跟踪变更仅为四份说明文档。

- Windows：`dist/windows/MDcard.exe`。
- Linux：`dist/linux/mdcard`。
- 完整包与身份：`dist/latest.json`。

未修改发行目录中的任何文件、归档或 manifest，也未重新编译产品。仓库说明更新属于清理文档，随包说明仍是原来冻结的发行快照。此次未重新进行 GUI/功能验收；这里只证明最新完整包和运行目录在清理前后保持完整、哈希不变。当前功能、平台与性能结论继续以工作台最终验收的原始证据及其限制为准。
