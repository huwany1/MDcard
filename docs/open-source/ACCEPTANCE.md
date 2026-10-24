# MDcard 源码开源验收

本报告区分源码开源、历史产品验收与本次构建。本次G01—G08对应的C01—C08已逐项通过。源码与许可证已匿名读取、完整clone和独立复核，通过[PR #1](https://github.com/huwany1/MDcard/pull/1)保留原main祖先并合并；具体最终main SHA以PR的merge_commit_sha和主代理交付中的远程核验值为准。本次不重新打包旧发行资产，不声称产品已全面验收。

输入基线：本地 `a73f3676b0129126fd71ea340355a4aa562adc86`；原公开 main / 发行标签为 `2e8279f42db82c999f020199bcc16731ede9559f`。清洁导入保留原 4 个公开提交，本地完整历史另行备份，不向远程推送本地其他分支和标签。

| 标准 / 目标 | 结果 | 可复查证据与边界 |
| --- | --- | --- |
| C01 / G01 范围 | 通过 | 本地[范围](SCOPE.md)、[638 路径逐项来源/排除](import-inventory.json)、实际远程 commit tree 已由匿名完整clone核对（初次502文件），不靠ignore推断；[最终公开文件清单](public-files.tsv)记录文件、mode、blob和SHA256（清单自身除外） |
| C02 / G02 AGPL | 通过 | [完整 LICENSE](../../LICENSE)、[许可边界](../LICENSING.md)；所有自有 Cargo/npm 根元数据和 lock 根一致 AGPL-3.0-only，vendor/第三方例外明确 |
| C03 / G03 授权 | 通过 | [第三方表](../THIRD_PARTY.md)、原 OFL/MIT/Apache/NOTICE、[旧 MIT](LEGACY_MIT_LICENSE.txt)、[原发行资产身份](remote-assets-before.json)；旧 MIT 不撤销，原包不改写 |
| C04 / G04 公开风险 | 通过 | [敏感信息与二进制核查](SECURITY_REVIEW.md)、[94 项邮箱误报](sensitive-findings.json)、[30 项缓存 digest 逐一重算](gitleaks-reviewed-findings.json)；Gitleaks全实际公开可达链已扫描、原始exit1、30项精确已核实误报，无已知未处理项；不是 0 命中或绝对无泄露保证 |
| C05 / G05 历史资产 | 通过 | 原远程祖先、tag 与 Release 记录已保存，RELEASE_NOTES.md / SHA256SUMS.txt blob 原样保留；[公开后资产对照](remote-assets-after.json)的Release正文/ID、3个资产ID/size/digest/URL与before完全一致；[匿名下载HEAD](checks/release-download-heads.json)均HTTP200且长度一致 |
| C06 / G06 干净开发 | 通过 | [命令/环境/exit](checks/local-validation.json)、[桌面日志](checks/desktop-linux-and-windows-gnu.log)、[Windows renderer](checks/renderer-windows.json)、[Linux npm/进程测试](checks/renderer-linux-install-and-process.log)、[网站](checks/website-browser-summary.json)；[39ff的远程CI](checks/ci-source-39ff.json)三jobs全部success，含原生Windows MSVC、Linux与renderer/site；完整导出资源/全格式GUI不在本轮复验范围 |
| C07 / G07 文档 | 通过 | 中文[README](../../README.md)、[贡献](../../CONTRIBUTING.md)、[开发入口](../DEVELOPMENT.md)、[导航](../README.md)及有效 Issues；全 Markdown 相对链接已检查，缺失历史证据改为明确本地引用 |
| C08 / G08 公开交付 | 通过 | [匿名source/LICENSE字节核验](checks/anonymous-source-39ff.json)、[主代理匿名读取](checks/master-anonymous-http.json)、匿名完整clone和全树复核；author/committer映射huwany1+noreply；[源码CI](checks/ci-source-39ff.json)通过，最终main SHA从远程重新取得，与主代理交付一致 |

## 本次实际检查

初次验证基线为`3126c1be986a9a734f0e4747430e3b1167986d95`；最终功能源码为`39ffb198f77123bf8924e2b07122ebd0a2b12b56`，此后的提交仅补文档和证据。主代理对3126阶段独立比较28份src/*.rs + build.rs，去掉SPDX/版权头并规范换行后均与本地基线相同。随后原生Windows CI发现branding.rs的严格lint，改用without_provenance保留Win32资源ID1并去掉多余return；因此最终Rust源码不能再描述为全部字节相同。renderer 为处理许可证组合明确禁用 ELK，保留默认图表并修复错误后的配置残留。

- 干净 `--no-local --single-branch` 克隆，仅使用候选可达历史。工具为 Rust 1.92.0 / Debian 12、Node 24.21.0、Java 21、Windows Edge 155；允许复用 Cargo/npm 缓存，Linux Node 是已安装工具的只读挂载，不是未公开源码输入。
- fmt、clippy（-D warnings）、cargo test：222 passed / 0 failed / 3 ignored；Linux release 与 Windows GNU 交叉 release 成功。GNU交叉编译不等于原生MSVC或Windows GUI验收。最终39ff远程CI另在原生MSVC完成fmt/clippy/tests/release，Linux同样通过；[最终Linux日志](checks/desktop-linux-final-39ff.log)保留本地全量复验。
- Linux release 在 Xvfb 实际启动并 3 秒自动退出，exit0；SHA256 `036ec2c72f7675bca5eed1ee1104d4b438b9dac05b10653f709968bae5c1df1b`。仅证明该环境启动，不代替完整 GUI 矩阵。
- Windows 干净 renderer npm ci、bundle build、固定哈希 PlantUML jar 获取及 30 个真实引擎/主题案例通过；缓存、7 个坏/恶意输入、错误恢复通过。构建输入断言无 elkjs，指定 ELK 明确失败；后续 Dagre/sequence/mindmap 正常。
- Linux 干净 npm ci 和 bundle 成功，5/5 进程所有权、继承管道、取消、超时和旧目标保护测试通过。Windows 同一进程测试按平台跳过，不计为通过。
- 网站全新克隆无需 Rust/私有 dist 即可构建与 HTTP 200；六宽度、键盘/交互及真实 200% 标签页缩放检查通过。SKIP_PERFORMANCE=1，因此没有新的性能结论，没有重新部署线上网站。
- xmldom 间接实例精确固定为0.9.12，MathJax 真实生成有效 SVG；[依赖风险](DEPENDENCY_RISKS.md)保留12项 npm audit条目及剩余处理范围。

## 保留的限制

原始 Chromium 下载在本机网络长时间无进展后中止，日志保留；随后GitHub Linux runner已从官方CDN成功取得Chromium156并完成30案例。实际引擎回归采用文档支持的显式系统 Edge 路径。最小 renderer 准备与完整桌面导出资源准备分别写清，不能用最小步骤暗中依赖维护者 dist。完整新发行包、9格式 GUI导出、macOS、Wayland、所有DPI/驱动、稿件崩溃恢复、物理显示延迟和全部产品安全未在本轮证明。旧产品报告仅保留其原有环境/构建身份。

尚存依赖风险不等于发现秘密；C04 的处理范围是秘密与未经授权私人内容。本次开源完成后仍需独立维护依赖升级、产品安全和发布验收。

## CI 发现与修复

首次远程运行40091251272确实失败：Windows clippy检出整数资源ID指针表达式和多余return；renderer取消测试的/proc读取在进程消失时返回ESRCH。后者产品代码本就处理ENOENT/ESRCH，遗漏只在测试helper，现按同样规则接受进程已不存在。修复分别见be14bf0和a683cf0，保留所有旧run，不把旧失败说成通过。

现有Git PAT不含workflow scope，首次Git ref push被GitHub拒绝；随后使用已授权GitHub connector正常create_branch发布同一已扫描commit和workflow，没有更改远程权限或绕过权限检查。后续普通文档/源码增量由标准Git快进推送。

39ffb19进一步修复Windows短路径父目录别名下的真实阅读状态迁移问题：旧文件重命名后已不存在，不能重新canonicalize它来选择旧持久键；改为沿用已绑定doc.id的record.path。Unix目录symlink回归先失败（exit101、旧key未清理），修复后通过，reading_state 42/42通过，GNU all-targets clippy通过；主代理独立复核实现和回归。此源码修复、ELK隔离及xmldom修复没有重新打入既有R3/R29包，旧下载仍按原字节及历史边界识别。

最终必要CI证据为[39ff push run 40092696769](https://github.com/huwany1/MDcard/actions/runs/40092696769)：Linux、原生Windows与renderer/site三jobs均completed/success，主/子代理独立确认。同SHA重复PR workflow无需等待来重复证明；后续纯文档/证据提交和main merge触发的CI如排队或运行中，不计为已通过。报告不会因写入CI结果而把新排队任务说成完成。
