# 阅读精确恢复交付（2026-10-24）

> 本文保留 R29 阅读精度交付的原始身份和证据。后续 Windows 已发布 Icon Suite R3，当前包与图标显示验证见[图标验收](ICON_INTEGRATION_ACCEPTANCE.md)；Linux 继续使用本报告的 R29 包。实际分平台运行入口以 `dist/latest.json` 和[项目 README](../README.md)为准。

阶段 A「文字/源码逻辑行级恢复」和阶段 B「大幅修改后的精确重定位」分别通过本轮约定支持范围的验收。完整 [38 项表](READING_PRECISION_ACCEPTANCE.md) 保留原编号和限制；[阶段 B 独立记录](READING_PRECISION_INDEPENDENT_B.md) 保留逐轮失败及复验。非文字节点、证据不足和滚动边界不冒充文字精确。证据归档另行逐成员校验，交付哈希清单与运行包并列保存。

## 源码、构建与完整包

产品提交为 `40b5e3bb16e1658e333cf2fddf965a22d4e878ff`，包内源码/文档快照为 `8c09f82daad47f623d9020b2370dfe57123ba80f`。阶段 A 初始实现提交 `857bb11c6012d0ec8fe2023eda67a07d4ebc69eb`，后续副窗格与首次挂载缺口在当前版本完成修复；R10 旧包保留历史勘误。阶段 B 最终定位组件提交 `f579461cfcfc28797bce2f8b8b89e6c64742ae4f`，R29 增量仅涉及诊断发布与观察器关闭刷新，最终产品身份仍以上述40b提交为准。

`artifacts/reading-precision/stage-b-r29-source.json` SHA256：`655cfdafcdd4feae4d3199206b49a39ba72c497966adec6378bcfe32d2210b55`。82个列入输入、70个实际本地编译输入、5份实际dep-info和3个二进制已独立核验。`locales/ui.json`、`src/text_files.rs`的CRLF/LF按Git文本属性核对，其余原始字节完全对应；不宣称可重复构建的二进制位级相同。

R29已正式发布；完整包位于 `dist/`，默认运行目录为 `dist/windows`、`dist/linux`，包含Node、Chromium、Java、Pandoc、引擎、字体、许可证和源码快照。完整解压后运行根目录 `MDcard.exe` 或 `mdcard`，保留同目录资源。`dist/latest.json`现指向R29 stable。发布记录见 `docs/releases/2026-10-24-r29.json` 与 `evidence/release-r29-2026-10-24/promotion.json`；下列完整包与程序哈希保持验收时字节不变。历史候选路径已清理，旧报告中的原始路径可从证据ZIP按同名成员提取，清理去向见 [清理记录](RESOURCE_CLEANUP_2026-10-24.md)。

| 对象 | SHA256 |
| --- | --- |
| Windows ZIP | `efabd1e02c64715b55e2c0eedf2e5b5f542f761ee370d121a2c5086889692412` |
| Linux tar.gz | `3771f142d7b8f1e6bec21059c1d82d559b33bcc62aea13c82fd20fddaab509ab` |
| Windows应用 | `face40f4af716c004b39e4ce992af088dedda171f924a73df8cf8504eefb67e1` |
| Linux应用 | `b438bd9e042366a77c63bcb9f6ccc989414119788959086cec98114f2c6cd9cf` |
| Windows实际测试PE | `5555163c48468d8e900ab66a579f4a5c8d8f7bb998949175adc0d75786ca9de1` |

包内文档是构建时验收前快照，最终验收报告位于包外并绑定这些不可变哈希，避免为嵌入自身最终哈希反复打包。归档逐成员审计及实际解压运行前后，Windows22059、Linux22003个payload均SHA/size/精确集合通过；不跟随链接，不从旧dist借用运行时。原生Linux首次解压末尾误查不存在的`runtime/browsers`导致QA失败，全部已提取文件保留；随后仅校验现有提取目录的实际`runtime/chromium/chrome`并通过，没有重解压或修改产品。

## 阶段 A 结论

阅读锚点独立于光标/选区/焦点，原稿UTF-8、源码逻辑行、字素簇、实际视觉行分别处理。可映射文字在同布局按偏移、重排后按同字素及视口y恢复。普通/大Markdown和TXT、长段中末部、虚拟卸载重建、主副窗格、相同ID/相同revision不同正文、编码/中文/组合emoji、宽度/字号/换行/模式、正常与异常重启、全部打开入口和保存保护有本轮真实GUI与独立组件证据。

当前完整包Windows原8组全部通过，Linux原6组11命令全部通过；不是只启动程序或重用旧截图。Windows原生键鼠操作所属HWND，Linux用X11实际输入；诊断只读。主代理另核对冻结驱动、原稿字节、实际Galley坐标、逻辑行和实际截图。

Windows最终包40个未夹取文字观测最大y误差为**1.53125逻辑px**，7个旧新原稿oracle独立重解码通过；不是40份独立文档。另列2个夹取：原稿58240「粗」的document-end误差**87.625px**；large TXT副窗格889「l」的document-start误差**2.506240845px**。两者不计入≤2px未夹取分母。更完整R25同组件23组矩阵198个未夹取观测、5个夹取与所有中间失败继续保留。

Linux当前包独立审计含28个字体/宽度实际文字观测、4个独立编辑oracle和2个运行中导航取消。上方真实PNG加载后原字素1441误差1px/补偿1次，真实KaTeX加载后原字素1064误差0px/补偿2次；均观察2.1秒稳定区间，没有持续漂移。五张当前包截图已由主代理实际打开查看，字体缺字呈现继续作为限制。

异常退出的完整四格式/双视图本轮矩阵中，记录年龄1703–2168ms，最近成功写入完成年龄732–1445ms；不承诺异常退出绝对零丢失。正常关闭刷新通过实际最终计数验证。

## 阶段 B 结论

独立测试集在实现前冻结，1400个不同旧新版文档对，每对一个锚点：1000可判定、200歧义、200删除/无对应。533例字符编辑量达到旧文Unicode scalar长度30%，300例原内容块移动比例达到30%；可判定子集中分别300/300例，完整计算方式、分布、输入、答案和逐例结果保留。

1000/1000正确精确恢复，精确命中率100%、自动精确覆盖率100%、精确声明正确率100%；全集错误精确声明0/1400。400/1400降级（28.5714%）；超时、取消、崩溃、非法边界和缺失结果均0，分母完整保留。该结论只适用于本轮独立集，不承诺任意文稿零误定位。

原冻结集用当前完全相同的纯定位组件SHA `685a8475a8cf8314c05ae8bbf7a72ffdf099201613a9e992c8935366919f1af9`重新编译适配器执行；R29无定位算法变更。纯算法批测不验证GUI y坐标，也不冒充1400次GUI。另有8项独立原稿/Galley测试、11项结构测试/768公开变体，分别统计，不加入独立集分母。

最终包复验原45884→115252、762647、40557、763018等历史失败位置，真实900段前缀编辑/Undo和后续键入/一次撤销保留焦点与选区；重复、删除和无证据路径降级。搜索/目录在恢复匹配实际运行时由真实输入取消，重排后至少2秒未拉回历史位置。

## 性能与故障

最终候选和原264d42d功能基线使用相同745f2725观察器分别构建，普通、1MiB、10MiB及长段/重复输入SHA和原生输入序列相同。两平台48份完整原始观察、24格式行的诊断OFF/ON all/wheel P95均通过实施前20%/2ms增量预算；全部final flush/count及0队列/保留丢弃独立核验。六阶段P50/P95/max/n见原始数组审计；热捕获/计划/入队P95最大0.090382ms、单次最大1.969573ms，通过2/8ms预算。R29实际运行中三阶段取消上界56.119/52.702/74.829ms，通过100ms门槛。没有在测试后放宽阈值。

测量为墙钟；线程CPU未测，不宣称物理显示延迟或FPS。Windows nearest-rank、Linux floor(n*p)、独立Rust基准自己的分位数分别明确。索引各n=1；layout n=2000是末尾保留窗口；完成匹配为30个，不重复计算为60个。RSS采样不含renderer，也不等于分配器峰值。旧R25/R26诊断ON超预算、R28最终0B raw及首次QA路径错误全部保留，不以新的通过结果抹除旧失败。

Linux实际核心219通过/3忽略，Windows本机210通过/2忽略；独立state118默认（12主代理独立+106导入）及4专项/14变化，分别列数。旧状态默认路径迁移、损坏/未来版本拒写、权限与原子替换失败、容量512条/4MiB、多实例合并、单个/全部清除及迟到旧实例不复活均有证据，原稿和恢复历史保护保留。额外3项Linux/2项Windows忽略测试未执行，不加入通过数；具体名称保留在38项表及独立B记录。

## 证据入口与边界

主代理审计位于 `evidence/reading-precision/master/`：

- `b-r29-build-inputs-independent-audit.json`、`b-r29-commit-source-binding.json`：实际构建/测试及源码绑定。
- `b-r29-full-update-independent-audit.json`、`b-r29-six-stages-independent-audit.json`：原始性能与各分母。
- `b-r29-complete-package-archives-audit.json`：两个完整包全部归档成员。
- `b-r29-windows-package-independent-audit.json`、`b-r29-linux-package-independent-audit.json`：最终包真实GUI与独立原稿/坐标复核。
- `b-r29-package-async-and-visual-independent-audit.json`：实际异步补偿和已查看截图。
- `b-r29-extracted-windows-post-gui.json`、`b-r29-extracted-linux-post-gui.json`：实际包内运行后全部文件保持一致。

Windows验证范围为Windows11 26100 x64、125%DPI、i7-13650HX/RTX4060Laptop；Linux为Debian12/X11/Xvfb/Openbox/Mesa软件OpenGL。Mac、Wayland、其他桌面/硬件未验证。图片/公式/图表及隐藏语法无法可靠映射时按节点或块降级。字素边界正确不代表字体完整呈现所有emoji。GUI单文件仍限制16MiB，源码索引64MiB/50000节点不是打开上限承诺；超预算证据、删除、歧义、无关原稿均保守降级。未命名草稿正文跨重启恢复与外部移动自动身份识别未在本轮实现。
