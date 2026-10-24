# 页面主要宣传与证据映射

核对日期：2026-10-25。0.1.0 发布准备使用同一份公开冻结源码；准确来源与文件哈希由 src/data/releases.json 绑定。原有产品截图与样例保留其真实历史来源，不替代新发行包的功能与纯净系统验收。

| 页面表述 / 位置 | 当前依据 | 本次核对与限制 |
| --- | --- | --- |
| Rust 原生 Markdown 与文本桌面编辑器 / 首屏 | `README.md`；`Cargo.toml`；`src/main.rs` | 原生指 Rust/egui 界面。公式图表与导出使用随包 Node、Chromium、Java、Pandoc，不称纯 Rust 排版。 |
| 面向中文技术写作者 / 首屏 | `docs/MATURITY_ROADMAP.md`；README 的中文、代码、公式、导出能力 | 产品定位与本页任务场景；不虚构用户调研、客户数量或推荐评价。 |
| 源码 / 分屏 / 阅读 / 演示与写作段 | README；本轮 `evidence/product/{write,split,read}.png` 与 JSON | 既有真实 Windows 应用截图，同稿切换；拍摄身份保留在素材来源记录。网页演示单独标注身份。 |
| 跨文件分别记住最后、最远、已读 / 阅读段 | `docs/READING_STATE.md`；`docs/READING_PRECISION_DELIVERY.md`；`docs/READING_PRECISION_ACCEPTANCE.md` | 滚到底不自动已读；大幅改写、歧义、非文字与边界夹取存在降级，不宣传任意文稿精确恢复。 |
| 表格结构操作与源码联动 / 整理段 | `docs/STRUCTURAL_EDITING.md`；README；本轮 `evidence/product/structure.png` | 普通正文预览只读；支持结构块才有联动编辑，不把整篇阅读视图说成富文本编辑器。 |
| 公式、Mermaid、本机运行时 / 整理段 | `docs/MATH_DIAGRAMS.md`；README；本轮 split 真截图 | 未宣称所有输入均成功；本次稿件真实显示 Q=C+S+E 与 Mermaid。 |
| 搜索 / 整理段 | `docs/GLOBAL_SEARCH.md`；本轮 `evidence/product/search.png` | 不引用旧 1600 文档 benchmark，不承诺通用毫秒级搜索。 |
| PDF、Word、离线HTML、PNG/JPEG、EPUB、Marp三格式 / 交付段 | `docs/LOSSLESS_EXPORT.md`；`docs/BUG_AUDIT_FOLLOWUP_2026-10-22.md`；README | 本轮实际同稿 HTML/PDF；其余格式依据已有实现/验收，格式边界明确，不宣称逐像素一致或PPTX可编辑文字。 |
| 真实原稿→效果→交付 / 整条旅程 | `website/evidence/product/provenance.json`；`gui-export/provenance.json`；`exports/provenance.json` | 源稿统一 SHA256 `24d49068a1f115a24bbbf4525c7a774015bc526f9e0c84836bd51f0d9dbff3a6`。真实 GUI 修改/保存，真实原生对话框导出 HTML，随包引擎另生成 PDF。 |
| 原稿本机处理，不上传服务 / 本地段 | README；`renderer/worker.mjs`；`renderer/export.mjs`；`src/rich_ui.rs` | HTTP(S) 图片、点击外链及导出远程资源可能联网；没有写“所有功能永不联网”。我们编写的站点源码无追踪脚本，Cloudflare托管可能插入安全挑战脚本，不承诺线上HTTP零额外脚本。 |
| 未保存提示、原子保存、外部修改检测 / 本地段 | README；`src/document.rs` | 当前没有自动保存与崩溃恢复，明确说明不能替代断电/崩溃备份。阅读状态持久化不等于稿件恢复。 |
| Windows 优先、Linux x64 / 下载段 | `dist/latest.json`；平台 promotion；`docs/ICON_INTEGRATION_ACCEPTANCE.md`；`docs/READING_PRECISION_DELIVERY.md` | 0.1.0 两平台使用同一冻结源码并分别校验二进制与归档；Linux 验收 Debian 12/X11，不扩大为全部发行版或 Wayland。没有 macOS 正式包。 |
| 包大小、构建、更新、SHA256 / 下载段 | `src/data/releases.json`；`scripts/releases.mjs` 当前完整包实算 | Windows 603792422 bytes，Linux 655893443 bytes；包含运行时，不称极小包体。source_commit 与 binary_source_commit 分开保留。 |
| 16 MiB 单文件上限 / 本地段与FAQ | README；`src/document.rs` | 不宣传无限大文档、零内存成本或无上限性能。 |

审查结果：没有用户评价、下载量、合作品牌、未经测试性能数字或注册商标声明。初稿曾包含 ®，因缺证据在独立复核时删除，最终页面不含此标记。
