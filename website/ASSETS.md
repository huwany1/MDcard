# 素材来源与连续视觉设计

本页没有使用生成式界面图或素材站图片。品牌符号、真实截图和公开示例均来自当前 MDcard 项目。

| 网站素材 | 原始来源与处理 | 身份 / 用途 |
| --- | --- | --- |
| `src/assets/favicon.svg` | `assets/branding/mdcard-icon/svg/mdcard-color.svg` 原样复制 | 卡片形 M，#2D5BCA。导航、favicon，未重绘图标。 |
| `src/assets/brand-white.svg` | `assets/branding/mdcard-icon/svg/mdcard-white.svg` 原样复制 | 蓝底下载区的白色版本，沿用相同几何与透明负空间。 |
| `src/assets/favicon.ico` | `assets/branding/mdcard-icon/ico/mdcard-color.ico` 原样复制 | 现有多尺寸 ICO。 |
| `src/assets/product-{split,write,read,structure,search,export}.webp` | `evidence/product/*.png`：本次实际 Windows GUI；Pillow WebP quality84 / method6 | 1700×1075 原始物理像素，120DPI，1360×860逻辑窗口。只压缩，不拼接或改写 UI。 |
| `src/assets/share.png` | 本页专门制作 1200×630 分享图：现有品牌蓝图标、纸白背景与主标题 | Pillow+系统字体排版，非产品UI截图；不冒充真实产品界面。 |
| `src/assets/exports/manuscript.html` / `.pdf` | 当前 Windows 正式包导出引擎；HTML另有GUI原生导出链 | 源稿哈希与 `samples/从原稿到作品.md` 一致；公开示例，不是安装程序。 |
| `src/assets/exports/manuscript-html.zip` | Python标准ZIP封装上述HTML，单一成员与原始导出字节完全一致；manifest独立记SHA | 仅作传输，避免在线托管HTML附加安全挑战脚本改变离线成品；不是新增产品导出格式。 |

Windows 产品程序 SHA256 为 `32b781a8b0915f96443b5e827281fc4fc395dabf04a0f340d85c048c03f696c4`，构建 `icon-suite-20261024-r3`，binary source commit `9fe090ea938a35b9f5d14ea647d7377b65fd5ab4`。原始逐视图 JSON 与 provenance 同目录保存，可从实际 HWND、mode、revision、document_path、selection 追溯。

视觉构图由产品本身推导：卡片形 M 对应文档容器，首屏把真实分屏界面放在两张轻微错位的稿纸上；同一源稿进入网页三态和四个章节，章节号、页边批注、纸白/墨灰与蓝色校对记号沿途重复。下载区把相同卡片符号反白，表示稿件走到交付。这是本页的设计选择，不声称全球唯一。

中文大标题采用系统宋体序列，正文采用系统无衬线序列，代码采用系统等宽字体；没有远程字体请求。品牌资源设计与许可证见原仓库 `assets/branding/mdcard-icon/DESIGN.md`、`LICENSE`。产品字体与第三方许可留在完整包内。

网站自有源码、样稿、品牌图形、图片、截图中的自有部分及导出排版统一由根目录 `COPYRIGHT` 授予 AGPL-3.0-only。导出 HTML/PDF/ZIP 中嵌入的字体及其子集仍采用 SIL OFL 1.1，详见[第三方清单](../docs/THIRD_PARTY.md)；样例原字节及哈希保持不变。网站构建在输出中附上 `LICENSE`、`COPYRIGHT`、`docs/LICENSING.md`、`docs/THIRD_PARTY.md` 和 `assets/` 下字体 OFL 与来源表，单独分发示例时也应携带这些字体声明。

移动端产品画面保持完整 UI，点击可打开原分辨率 WebP；网页演示为可读的独立窄屏布局，不把桌面真实窗口强行描述为原生手机版。

2026-10-25 发行更新说明：现有产品截图和导出样例保留其原始拍摄、生成与哈希记录。它们展示真实 Windows 应用的界面与场景，没有改标为新 0.1.0 二进制的截图。新版包的来源、运行时和功能验收由独立发行记录证明。
