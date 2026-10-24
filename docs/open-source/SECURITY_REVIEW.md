# 发布前敏感信息与媒体核查

扫描对象是候选公开分支全部可达历史和实际索引 blob，包括旧 main 祖先，不是整个本地私有开发库，也不通过 .gitignore 决定是否扫描。本地私有分支不推送，不做强制推送或远程历史删除。

使用两层检查：scripts/audit-publication.py 对 staged blobs 与指定公开祖先做完整文本规则扫描和二进制清单；Gitleaks 8.30.1 对最终公开链 `--log-opts=HEAD` 扫描（工具官方发行 ZIP 已核对官方 checksum）。规则覆盖私钥、常见服务 token、JWT、带凭据 URL、secret assignment、个人主目录与邮箱。命中不输出秘密原文，仅保存路径、blob、行号和值哈希，逐项处置见[sensitive-findings.json](sensitive-findings.json)。

初次候选扫描 480 个 unique blob、94 项邮箱模式命中，逐项核对全部位于第三方 copyright/contact 或 SSH git 源码地址，均为必要/公开上游声明，保留。没有以大范围 allowlist 忽略许可文件；最终扫描统计与修订范围见[验收报告](ACCEPTANCE.md)。

## 二进制与非 UTF-8 内容

- 9 份 TTF：逐份读取 fontTools name 表（版权、家族、license URL）；上游 OFL 字体及正确改名的静态实例，不是私人稿件。
- SVG、PNG、ICO、图标 preview：项目自有卡片形 M、光学尺寸和透明背景预览；保留可编辑源。无可执行嵌入外链/脚本的图标声明有原设计验收记录。
- 网站六张产品 WebP：主代理逐张独立查看，均为“从原稿到作品.md”合成演示稿。导出面板画面含项目 QA 目录的绝对路径，为已核实的合成测试路径，不含用户主目录、令牌或私人稿件，也不是开发依赖。
- share.png：项目自有品牌图标、标题、纸色背景组成的分享图，不是私人截图。
- 网站 PDF：pypdf 检查文档题名、Creator/Producer/日期及抽取文字；对应公开演示稿，没有账户、用户名或私人文稿元数据。
- HTML ZIP：只含一个“从原稿到作品.html”成员；内容是相同公开导出。manifest 记录其字节与 SHA256，不含安装包、目录遍历或其他文件。
- tests/fixtures/encodings：按 manifest 中指定 UTF-8/UTF-16/GBK/GB18030/Big5/Shift-JIS/EUC-KR/Windows 编码解码的合成短文本，包含中日韩/俄文、emoji 与换行测试；不作为未经检查的“二进制”跳过。
- PlantUML-MIT-LICENSE.txt：由上游 Java MIT 构建输出，保留原字节；少量非 UTF-8 显示字符位于上游宣传/许可输出，不是凭据。正文 MIT 授权和免责声明已人工核对。
- Cargo/npm lock 中 integrity、SHA256、Git object ID 是完整性值，不是凭据。没有把随机字串一律当误报忽略。

本轮未发现已知待处理秘密或未经授权私人内容。扫描只能降低风险，不能证明绝对无泄露；贡献者仍须检查新稿件、截图和部署数据。发现秘密时应先停止公开相关内容、撤销凭据并按具体可达历史处置，不能只在最新版本删一行就宣称历史已清理。
