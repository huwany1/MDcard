# MDcard 静态产品页

公开产品页：https://mdcard-document-journey.wholeangel11.chatgpt.site/ 。源码为静态 HTML/CSS/ES modules，不需要账户、数据库、Rust 或维护者部署凭据。

在仓库克隆后的 website 目录执行（Node.js 24）：

```sh
node scripts/build.mjs
node scripts/serve.mjs
```

打开 http://127.0.0.1:4173/，Ctrl+C 结束。构建输出 website/dist，使用相对资源，可交给普通静态主机。不需要原始 .openai 托管配置。npm run build / npm run preview 是相同入口。

## 浏览器检查

```sh
npm ci --ignore-scripts --no-audit --no-fund
npx --no-install playwright install chromium
```

另开终端启动预览；然后 BASE_URL=http://127.0.0.1:4173/ node scripts/verify.mjs。若使用已安装 Chromium，设置 CHROMIUM_PATH；未设该值时脚本使用 Playwright 默认浏览器。Windows PowerShell 用 $env:BASE_URL 和 $env:CHROMIUM_PATH。结果写入本地 evidence，默认不公开；其中可能有本机路径、QA 状态和截图。验证脚本的真实浏览器 200%缩放依赖临时扩展，未运行项不得写成通过。

## 发布数据、素材与边界

src/data/releases.json 记录 0.1.0 的两平台来源、归档体积和哈希，data/downloads.json 保存实际公开 URL。普通 build 不依赖上级 dist；scripts/releases.mjs 接受显式发行索引路径。发布准备阶段的 URL 为 null，页面明确暂停下载；本地准备不代表已经上传或上线。更新链接时先核验发行元数据及下载身份；修改源码或本地构建不代表已经发布。

公开画面和演示素材的来源见[ASSETS.md](ASSETS.md)，功能陈述边界见[CLAIMS.md](CLAIMS.md)。样例在 samples/从原稿到作品.md，HTML/PDF/ZIP是该公开稿件的真实导出。网页三态仅解释视图，没有在线编辑或本地文件能力。自有网站源码、文档、样稿、设计及图片等资产统一为 AGPL-3.0-only，见根目录 [COPYRIGHT](../COPYRIGHT)；第三方素材及内嵌字体保留原许可，见[例外清单](../docs/THIRD_PARTY.md)。构建输出同时携带项目声明与字体 OFL。

原始浏览器报告、部署日志、安装状态和旧验收记录不属于当前源码输入。运行结果写入被忽略的 evidence；本地构建和浏览器通过不代表线上发布。

可执行 `node scripts/verify-samples.mjs` 验证真实样例下载；可选 `CHROMIUM_PATH`，默认使用 Playwright 浏览器。`node scripts/export-sample.mjs` 从当前 renderer 生成新的 HTML/PDF 到 evidence，先按开发说明准备完整导出运行时；不会覆盖受控样例。`python scripts/prepare-html-download.py` 为已受审 HTML 生成 ZIP 传输封装。

`python scripts/verify-public-downloads.py --page-links <captured-links.json> --out <local-report.json>` 是可选公开下载复核，需要 Python requests（`python -m pip install requests`），会流式下载大体积安装包计算 SHA256；普通网站构建与浏览器检查无需它。
