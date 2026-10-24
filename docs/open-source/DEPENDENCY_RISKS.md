# 本次依赖检查与剩余风险

renderer 的 npm clean install 初次提示间接 @xmldom/xmldom 0.9.10 的弃用/安全警告，链为 mathjax-full 3.2.2 → speech-rule-engine 4.1.4 → @xmldom/xmldom 0.9.10，直接依赖原本已是 0.9.12。

根据[上游 0.9.12 修复发布](https://github.com/xmldom/xmldom/releases/tag/0.9.12)与[GHSA-6gmq-8vp8-gcm6](https://github.com/advisories/GHSA-6gmq-8vp8-gcm6)等公告，新增精确 override 到 0.9.12，锁文件去除旧嵌套实例。npm ls 确认直接/间接均为 0.9.12。重新执行 npm ci、bundle build、30 个实际渲染主题案例、缓存/错误恢复测试；不是只改元数据而不运行。

`npm audit --omit=dev --json` 本次由 14 项依赖报告条目（11 high / 1 moderate / 2 low）变为 12 项（10 high / 2 low / 0 critical），原始安全字段见[npm-audit-before.json](npm-audit-before.json)与[npm-audit-after.json](npm-audit-after.json)。这些计数包含传递传播条目，不等于独立可利用漏洞数。

剩余报告涉及 Marp/Puppeteer 下载工具链、extract-zip、proxy/get-uri/basic-ftp、zod 以及 Marp 间接 KaTeX 等；主 KaTeX 渲染依赖已为 0.19.0，但不因此推断所有间接路径安全。npm 自动建议部分是破坏性的 Marp 主版本改变/降级，未在源码公开任务中直接执行。需在后续独立依赖升级与导出回归中处理，不能写为安全审计全部通过。

mathjax-full 3.2.2 仍有上游 v4 替代提示，本轮未迁移。Chromium 下载曾因网络长时间无进展而中止，保留失败记录；本轮实际 Windows 引擎检查使用已安装 Microsoft Edge 155、系统 Node 24.21.0、Java 21，通过文档中明确的 MDCARD_CHROMIUM / MDCARD_JAVA 设置，不依赖维护者私人稿件或未公开源码。

本次没有对旧 Release 包进行依赖替换或重新打包；旧包继续保留原 SHA256、许可证和下载地址。公开源码及干净构建通过不等于产品全部安全、所有导出格式或所有平台已重新验收。
