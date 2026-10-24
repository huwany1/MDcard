# MDcard 发布页验收

验收基线：用户 G01—G08 / C01—C08；2026-10-24。最终公开v3页面、两平台完整下载和维护交付均已验证，C01—C08全部通过；主代理独立终审见 final-acceptance.json（历史本地证据：`evidence/master/final-acceptance.json`，未公开）。以下限定具体环境、观察方法和未测边界；没有未关闭的关键问题。

| 标准 | 状态 | 证据与当前限制 |
| --- | --- | --- |
| G01 / C01 定位清晰 | 通过 | 首屏含名称、类型、目标用户、价值、Windows入口、真实画面。主代理组织3名未参与设计的临时观察者，受控复测计时10005/10005/10015ms，三项定位均理解，实际原话见 `OBSERVATIONS.md`。复测为同批代理重复样本，首轮总曝光18–22秒；不宣传真人/首次严格10秒测试。 |
| G02 / C02 视觉设计完整 | 通过 | 最终公开页桌面整页（历史本地证据：`evidence/browser-final-deployed/desktop-full.png`，未公开）、移动整页（历史本地证据：`evidence/browser-final-deployed/mobile-full.png`，未公开）、首屏（历史本地证据：`evidence/browser-final-deployed/desktop-first-screen.png`，未公开）；同稿、章节与卡片构图贯穿，素材及构图依据见 `ASSETS.md`。已纠正首轮截图平滑滚动造成的空白与skip位置问题。 |
| G03 / C03 场景真实 | 通过 | `evidence/product/provenance.json`：当前正式 binary 真实编辑/保存/三态/结构/搜索/导出；`gui-export/provenance.json` 原生保存对话框实际生成HTML；`exports/provenance.json` 随包引擎生成 HTML/PDF。唯一源稿 SHA256 `24d49068...ff3a6`。 |
| G04 / C04 演示可用 | 通过 | 最终 `evidence/browser-final-deployed/verification.json` 的 interaction：鼠标/键盘/模拟触屏三态、同源、稳定高度和scrollY、焦点保留、减少动画；身份明确为网页演示。`evidence/browser-final-public/download-accessibility.json`另验证两个实际下载按钮可键盘抵达且302×52px。触屏为Chromium模拟，未做物理手机触摸。 |
| G05 / C05 下载一致 | 通过 | 组合下载核验（历史本地证据：`evidence/master/public-downloads.json`，未公开） full_platform_matrix=true、passed=true；实际公开v3页提取两href，与release JSON逐字段核对。无凭据完整GET：Windows603792422字节、Linux655893443字节，最终文件名/Content-Length/实收字节/SHA256全匹配；Windows未变身份复用先前完整GET成功证据，Linux在最终页首次完整成功。两平台同hash归档安全完整解压并启动通过，见 `evidence/master/install-windows/result.json`、`install-linux/result.json`；只验证安装/启动范围。 |
| G06 / C06 事实追溯 | 通过 | `CLAIMS.md`：逐主要表述对应README/功能/验收/包；明确自动保存、崩溃恢复、macOS未交付；正确说明网络、完整包大小、Linux边界与托管安全脚本；无虚构社会证明与性能。 |
| G07 / C07 响应式与效率 | 通过 | 最终 `evidence/browser-final-deployed/verification.json`：360/390/768/1024/1440/1920六宽度无全局横溢、图片有效；可见链接/按钮/summary≥44×44、正文/元信息对比≥4.5；真实200%标签页zoom、innerWidth720；本轮3次公网移动模拟LCP1852/1432/1576ms、CLS均0。设备/网络/CPU/缓存逐轮记录，控制台/pageerror为0。不是物理手机或真实用户性能数据。 |
| G08 / C08 可复现交付 | 通过 | 自包含源码构建通过，README/CLAIMS/ASSETS/OBSERVATIONS、发布数据与完整证据齐全；标题/摘要/分享图/favicon/sitemap/主要资源已验证。Sites最终v3已公开部署，主代理独立终审确认18个当前页面源码/素材/data文件与部署快照逐字节相同；最终下载区域截图（历史本地证据：`evidence/master/deployed/download-final.png`，未公开）已目视核对。源码/版本/部署ID见报告末尾，最终截图/交互/下载/已知问题已记录；本地4173预览已停止。 |

## 本地验证条件

Windows 11 26100，Intel i7-13650HX，16 GiB 内存；Chromium 156.0.8078.4，Playwright 1.58.2。390×844移动模拟、DPR2.75、触屏模拟；下载1.6 Mbps、上传0.75 Mbps、150ms延迟、CPU4倍减速、每轮新context且禁cache。分别保留本机HTTP预览和实际公开Sites网址结果；公网测试仍是模拟设备，并非真实用户数据。

200%检查通过 Chrome `chrome.tabs.setZoom(2)` 设置真正标签页缩放，并用 `getZoom=2` 确认，1440布局视口变为720 CSSpx；不是CSS zoom或简单DPR修改。关键触区、对比度、同源内容、控制台、no-JS平台信息均由脚本记录。

## 已知问题与边界

1. 发布状态已关闭：最终v3公开页面和Windows/Linux原始完整包均可访问，两个平台实际完整GET已验证。自有域名未配置，使用MDcard托管子域名；本地完成与公开上线状态分别保留，未把仅本地包当作公开下载。
2. 本次移动端操作和性能使用浏览器模拟；物理手机、Safari、Firefox及真实移动网络没有测试。主代理另用系统Edge独立复验，不扩大兼容范围。
3. 手机上真实桌面产品截图因完整展示而文字较小，可点击查看原图；网页三态使用独立可读排版。C01一位观察者已反馈此疑问，最终观察结果由主代理落档。
4. 懒加载仅用于折下真实截图；首屏真实图99KB、高优先级预加载。我们编写的源码没有远程字体、追踪代码或运行时框架；托管商可能附加安全脚本，见下一项。
5. 首次GUI导出设置枚举0实际为HTML，原先错误命名.pdf已改名保存为diagnostic；正式示例PDF来自真实PDF引擎并校验格式，GUI链已按HTML重跑，不能把扩展名当格式证据。
6. Cloudflare托管会对HTML响应附加约938字节安全挑战脚本；在线HTML预览如实保留，离线HTML ZIP保持原文件字节不变。主代理已从公开v2页真实点击下载ZIP并核对75891字节、SHA256和源ZIP逐字节一致，记录 `evidence/master/deployed-samples/sample-downloads.json`；该公开ZIP完整性问题已关闭。我们编写的页面源码没有追踪代码，托管HTTP响应不承诺零额外脚本。
7. 公网初轮Playwright API客户端对示例资源收到Cloudflare 403（5509字节挑战页），三次记录保留在 `evidence/browser-deployed/api-client-challenge.json`。实际浏览器同源fetch、真实下载及独立匿名requests均通过。搜索引擎收录与外部社交平台抓取效果尚未验证；页面只交付完整可用的SEO/分享元信息，不声称已收录。
8. Linux上传历史失败已关闭并保留诊断：首次并行请求全读后HTTP404，仅留下与另一同名请求时间对应的starter；没有token过期证据，不断言内部原因。后续单请求30秒write预算超时，只按精确ID/名称/state/digest清理本任务失败starter，保留Windows已成功资产。改120秒write预算后10:25:06 UTC上传完成且size/digest正确；checksum字节行尾差异修正为复用既有实际字节，10:28:20幂等发布成功。Linux最终匿名完整GET在10:36:06 UTC通过，未删除/替换任何已成功完整包。
9. 安装测试临时目录保留，非必需交付，不影响验收：`artifacts/release-page/install-windows` 与容器 `/tmp/mdcard-page-install-01a16`。主代理清理自建Windows临时目录的递归Remove-Item被自动审批以 `blocked by policy` 拒绝，未重试、未换工具绕过；后续Linux删除未执行。原始dist包与所有证据保留，不需要追加审批即可继续页面/发布任务。

## 状态的用法

本地完成、私有预览部署、公开上线是独立状态。最终只在实际发布并验证可公开访问地址后写“已上线”；自定义域名未取得时使用实际验证的托管子域名，不虚构自有域名或DNS配置。

当前公开地址：<https://mdcard-document-journey.wholeangel11.chatgpt.site/>。Sites源提交 `949b35ecd11edcfe7498aca16335588205aaacaf`，保存版本 `appgprj_6adc6e5b49ec81918648242a8e27412d~appgver_5bbb2ec1d8a481918b52059a5081d803`，部署 `appgdep_6adc741688808191b5e4a36e6bda31ed` 于09:03:29 UTC成功；public access revision2于09:12:29 UTC开放后完成匿名验证。自有域名未配置，使用与MDcard关联的实际托管子域名。

Windows阶段更新：源提交 `6bf1e83a22496c72ee86a5c797aaba0455767ccb`，版本 `appgprj_6adc6e5b49ec81918648242a8e27412d~appgver_ff474fa968548191bd3d8751c4cbfbe0`，部署 `appgdep_6adc8294ef508191bfc746f38af319e4` 于10:04:19 UTC成功，public地址不变；此版已开放Windows完整包、公开离线HTML ZIP，Linux保持暂不可下载。

最终双平台v3：源提交 `a09a0f0bac884b66ffaeb0b470311c94343b3726`，版本 `appgprj_6adc6e5b49ec81918648242a8e27412d~appgver_1df86be42cf48191bef34d0068d6513a`，部署 `appgdep_6adc89094b6c819181482d7d33928b38` 于10:31:49 UTC成功，public地址不变。主代理已从此最终公开页提取两真实href并完成两平台完整下载身份核验；最终Chromium六宽度/交互/3次性能与整页截图全部通过。托管源提交是部署时快照；部署后验收结果保存在本地Git证据目录，避免以更新报告变更已验证产品文件。

本地预览清理记录为 `evidence/teardown.json`：仅停止本任务拥有的4173服务，公开站点继续可用；安装测试临时目录按上述限制保留，没有绕过自动审批拒绝。
