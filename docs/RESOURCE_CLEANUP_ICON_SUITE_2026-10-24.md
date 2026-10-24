# 图标交付后资源清理（2026-10-24）

当前 Windows 为 Icon Suite R3，Linux 为 R29；正式程序、归档及 `dist/latest.json` 在清理前后均保持原字节。桌面 MDcard 仍指向 `dist/windows/MDcard.exe`。

## 实际清理

工作区删除 **135,396 个文件、11,650,942,261 字节（10.85 GiB）**；项目 Docker 编译缓存删除 **9,679 个文件、3,048,478,989 字节（2.84 GiB）**。合计 **145,075 个文件、14,699,421,250 字节（13.69 GiB）**。

其中 **3,193 个 Markdown** 是候选完整包、重复解压及依赖目录中的副本。有效源码文档、历史验收原件和唯一证据未按日期删除；新增[文档导航](README.md)，并修正旧报告的“当前版本”措辞。

| 清理目标 | 原因 |
| --- | --- |
| `artifacts/icon-integration-20261024/packages` | R1/R2已被R3替代；R3候选与正式包及运行目录重复，构建身份小型元数据另存 |
| `artifacts/icon-integration-20261024/master/最终解压 中文 空格` | 最终Windows完整包验收解压副本；原始报告/截图/夹具保留 |
| `artifacts/icon-integration-20261024/master/迁移后 新目录 中文 空格` | 迁移验收解压副本；正式长期目录已验证，迁移报告保留 |
| `dist/.before-icon-suite-20261024-r3` | 旧Windows展开回滚目录；同版本R29完整ZIP保留，可据原哈希重新解压 |
| `artifacts/icon-design/2026-10-24/browser-profile` | 已关闭的图标预览专用浏览器配置/缓存，不是用户浏览器配置 |
| `artifacts/icon-design/2026-10-24/browser-profile-no-extensions` | 已关闭的独立验收浏览器配置/缓存；JSON与真实截图保留 |
| `artifacts/icon-design/2026-10-24/unpacked-3404e05d5dcc` | 初稿资源包验收解压副本，最终可编辑套装和资源ZIP保留 |
| `artifacts/icon-design/2026-10-24/unpacked-af77c9454bff` | 最终资源包重复解压，原始套装与资源ZIP保留 |
| `assets/branding/mdcard-icon/tools/node_modules` | 图标生成工具依赖缓存，可由固定package-lock重装；运行时不依赖此目录 |
| `scripts/__pycache__` | 可重新生成的Python字节码缓存 |
| `artifacts/icon-integration-20261024/implementation/MDcard.exe` | 与正式Windows EXE同SHA256的实现侧副本；实际测试PE及日志保留 |

Docker 仅清空已核对挂载的 `mdcard-target` 卷 `/work/target` 内容；保留卷、容器、Rust/MinGW工具链及 `mdcard-cargo` 下载依赖。清理前无活跃编译或测试应用。

D盘执行窗口可用空间净增 11.12 GiB；该读数可能受其他进程影响。删除统计是文件逻辑字节，Docker虚拟磁盘不承诺立即缩小。

## 保留项目与理由

| 类别 | 原因 |
| --- | --- |
| 当前Windows/Linux展开运行目录及对应归档 | 运行与分发用途不同，均为用户当前正式入口 |
| Windows R29完整ZIP | 最近可回滚版本；其重复展开备份已经删除 |
| assets/branding/mdcard-icon | 原始12 SVG、27 PNG、3 ICO、预览和可复现工具；运行时图标不是缓存 |
| 原始截图、失败日志、测试PE、夹具及核准JSON | 历史观测不可由源码重新生成；保持当前验收与发布指针引用有效 |
| reading-precision/delivery证据ZIP | 大型但唯一的原始历史证据归档，不属于无用构建缓存 |
| 源码、Git、vendor、字体/许可证、renderer、tests、scripts | 产品、受控依赖、来源合规和复现材料 |
| docs下的合同、设计、验收与发布记录 | 保留操作规范、根因、独立结论及未验证边界；导航已分类 |

逐文件清单及原因见retained-files.tsv（历史本地证据：`../evidence/cleanup-icon-suite-20261024/retained-files.tsv`，未公开），分类统计见summary.json（历史本地证据：`../evidence/cleanup-icon-suite-20261024/summary.json`，未公开）。清单包含Git，反映生成时刻；后续提交会增加Git对象。

## 路径去向与验证

R1/R2旧完整包及R3重复候选已删除；其 `packages.json`、manifest、编译输入等小型身份记录按原相对路径保留在 `evidence/cleanup-icon-suite-20261024/retired-metadata/`。R3源码、资源与程序可从正式Windows完整ZIP提取；R29回滚目录可从保留的R29 Windows ZIP重新解压。历史报告中的被清理路径代表采集时位置，不将它们改指其他二进制。

真实截图、原始失败、源稿夹具、主代理脚本、最终独立报告、`release-clearance.json` 和发布记录保留原路径。没有修改正式包内文档/manifest，也没有重新编译或重打包。

清理前后均逐文件检查 Windows **22,257**、Linux **22,003** 个payload的精确集合、长度和SHA256，同时核对两份完整归档及EXE/ELF哈希，全部通过。证据：清理前（历史本地证据：`../evidence/cleanup-icon-suite-20261024/verification-before.json`，未公开）、清理后（历史本地证据：`../evidence/cleanup-icon-suite-20261024/verification-after.json`，未公开）。

显式删除计划和逐项结果：plan.json（历史本地证据：`../evidence/cleanup-icon-suite-20261024/plan.json`，未公开）、deletion-results.jsonl（历史本地证据：`../evidence/cleanup-icon-suite-20261024/deletion-results.jsonl`，未公开）、工作区结果（历史本地证据：`../evidence/cleanup-icon-suite-20261024/result.json`，未公开）、编译卷结果（历史本地证据：`../evidence/cleanup-icon-suite-20261024/docker-result.json`，未公开）。未扫描或删除真实稿件、用户配置、其他项目、系统临时目录或Docker全局资源。
