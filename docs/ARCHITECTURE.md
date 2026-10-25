# 维护地图

MDcard 是 Rust/egui 桌面程序，Markdown 或解码后的文本始终是正文真值。网站是独立的静态产品演示，不参与桌面应用构建。

| 层 | 主要入口 | 维护约束 |
| --- | --- | --- |
| 文档与磁盘 | `src/document.rs`、`encoding.rs`、`text_files.rs` | 编码、BOM、换行及未修改字节；原子保存、外部修改检测；单文件 16 MiB |
| UI 与结构操作 | `src/main.rs`、`structure*.rs`、`source_layout.rs` | 源码、结构操作共用撤销历史；普通预览只读 |
| 大稿与语法 | `large_document.rs`、`syntax_cache.rs`、`vendor/egui_commonmark` | Rope、增量解析、按视口布局与缓存；vendor 修改及上游许可一起保留 |
| 阅读位置 | `reading_state.rs`、`reading_precision.rs`、`reading_geometry.rs` | 文件/Buffer/打开代次隔离；字素与块级恢复分开；歧义和预算不足降级 |
| 工作区搜索 | `search.rs`、`search_ui.rs` | 有界索引、正则预算、替换预览及冲突保护；多文件补偿事务不等于文件系统原子事务 |
| 公式图表 | `rich_nodes.rs`、`rich_ui.rs`、`rich_svg.rs`、`renderer/worker.mjs` | JSON stdin/stdout 子进程；本地引擎、SVG 清洗、有限缓存和取消；不运行 HTTP 服务 |
| 导出 | `export_ui.rs`、`process_tree.rs`、`renderer/export*.mjs` | 点击时冻结正文/样式；独立任务、临时输出和进程树回收；提交开始后的失败须核查目标 |
| 语言与品牌 | `localization.rs`、`branding.rs`、`build.rs` | 字体、翻译、PNG 为编译输入，Windows ICO 交给 rc/windres |
| 网站 | `website/scripts/build.mjs`、`website/src/` | 直接复制受控截图、样稿和导出样例；不读取上级 dist 或私有部署配置 |

根项目测试覆盖应用内部逻辑；`tests/*/Cargo.toml` 是直接引入真实源模块的独立回归工程，各自保留锁文件。统一入口见[开发说明](DEVELOPMENT.md)。原生 GUI 和性能工具在[工具说明](../scripts/README.md)中单列，运行结果仅适用于实际环境。

完整第三方来源见 [THIRD_PARTY.md](THIRD_PARTY.md)。原始 OFL 字体同时是静态实例生成输入和导出字体来源；不得仅凭“未被 include_bytes 直接使用”删去。Chromium credits、Node/PlantUML 声明和 vendored 源码许可属于分发及维护输入。
