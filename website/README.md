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

src/data/releases.json 是当前正式 Windows R3 / Linux R29 的构建输入，data/downloads.json 保存公开 URL，普通 build 不依赖上级 dist。维护者更新正式包时，可使用 scripts/releases.mjs 从自己的完整产品包重新计算后审核，不能把没有上传的本机路径写成下载地址。现有 GitHub Release、资产与 SHA256 保持原样。本次未重新部署线上产品页。

公开画面和演示素材的来源见[ASSETS.md](ASSETS.md)，功能陈述边界见[CLAIMS.md](CLAIMS.md)。样例在 samples/从原稿到作品.md，HTML/PDF/ZIP是该公开稿件的真实导出。网页三态仅解释视图，没有在线编辑或本地文件能力。自有网站源码 AGPL-3.0-only；既有产品包仍按随附许可识别。

旧的原始浏览器报告、部署日志、安装状态与一次性上传脚本不公开；[本次开源报告](../docs/open-source/ACCEPTANCE.md)给出范围和检查。旧 ACCEPTANCE / OBSERVATIONS 只记录历史，不能作为本次重新验证或部署声明。
