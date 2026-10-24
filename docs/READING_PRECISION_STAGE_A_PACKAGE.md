# 阶段 A 包与独立复验记录

> 后续范围勘误：R17真实验收发现普通副窗格源码视图缺少文字几何，R19又发现large TXT首次历史挂载的同布局raw偏移误差4.756226px。上述旧包不包含这两项修复，不能据此宣称所有窗格精确恢复完成。后续修复及当前复验见 `READING_PRECISION_INDEPENDENT_B.md` 和38项验收表；本快照的历史数据及包哈希保持原样。下文“当前”仅指R10历史快照；现行阶段A支持范围须结合READING_PRECISION_INDEPENDENT_B.md及完整38项验收表。

现行阶段A已由R29修复后的实际完整包复验通过，阶段B亦单独满足独立评测与GUI门槛，见 [最终交付](READING_PRECISION_DELIVERY.md)。以下R10旧数据和旧包结论保留为历史快照，不能代替当前支持范围。

该快照交付已测范围内的原文字素/实际排版恢复；完整 A+B 目标仍未完成。尤其完整 update 预算、后台计算真正取消和大幅修改后的独立评测，继续按 `READING_PRECISION_ACCEPTANCE.md` 保留。阶段 A 功能提交为 `857bb11c6012d0ec8fe2023eda67a07d4ebc69eb`，文档格式快照为 `c28d1095bd0ec42f2e91833f095d3fac5828ede4`。

完整运行包位于 `artifacts/reading-precision/packages/precision-a-20261023-r10/`。内嵌 manifest 的 `sourceCommit` 为上述 c28d109，`binarySourceCommit` 为857bb11；两者产品源码相同。原编译28路径 SHA 与 Git 文本属性规范化后的提交内容逐一核对，记录在 `evidence/reading-precision/master/a-r10-commit-source-binding.json`，没有用旧基准提交替代实际实现身份。

| 归档 | 字节数 | SHA256 |
| --- | ---: | --- |
| `MDcard-0.1.1-precision-a-20261023-r10-windows-x64.zip` | 569160152 | `c6c80ab4ea3719ac57d16181d0348279e6df74802d4a238acaa093ccdbba64a1` |
| `MDcard-0.1.1-precision-a-20261023-r10-linux-x64.tar.gz` | 646310571 | `765d87f34a305dce5eeae943c66dcdc5cbf95aa411e682167ee37061096b8be7` |

主代理逐成员审计 Windows 22056 / Linux 22000 个文件，检查归档路径、无链接/特殊文件、清单一致、全文件长度/SHA、二进制身份和 Linux 必需可执行权限。完整运行时、renderer、字体、文档及许可证包含于包中，未用裸 exe 冒充完整包。原 `dist/windows`、`dist/linux` 和 `dist/latest.json` 历史记录保留。

审计输出 `evidence/reading-precision/master/a-package-r10-archive-audit.json`；Windows 程序 SHA `3dd8edc357f79edf28ffa377c9baa657f5f7040bc90e32b530c485a15b414374`，Linux程序 SHA `cfa218d0eafd5748b16ae55417fa6eb2ae2df285257ded11e50c0a422864e148`。

从与已审归档逐成员一致的包目录运行程序，并明确使用同包 renderer，复验：

- Windows：`a-package-r10-reflow-read` 9项、`a-package-r10-lifecycle-write` 5项、`a-package-r10-mapping-read` 1项均通过，覆盖真实宽度/字体/侧栏/分屏、正常/异常重启及预览映射。结果在 `evidence/reading-precision/master/`。
- Linux：`stage-a-package-rich-font` 与 `stage-a-package-async` 均通过。真实字体/宽度/Read-Write-Split和2.1秒尾段同字素稳定；PNG offset 400→1475、KaTeX399→619，实际字素误差均0 px，总补偿40/2且最终停止。结果在 `evidence/reading-precision/implementation/`。

以上包内15项 Windows断言是对阶段 A 原有25组227项运行的关键复验，不能据此增加唯一文档/算法锚点数量。Linux包复验保留其实际 X11/Xvfb/Openbox/Mesa 软件渲染边界，不扩大为其他桌面或GPU通过。阶段 B 的代码和证据将独立报告，不能使用本包完成声明替代。
