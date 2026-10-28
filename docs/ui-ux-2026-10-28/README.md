# MDcard UI/UX 优化交付

已完成展示层优化，并按“真实现状截图 → 高保真静态设计 → 分区实施 → 实机对照”交付证据。核心变化是工具与导航分组、清晰的文档身份/路径层级、紧凑保护状态、正文优先的比例、统一工具命中区，以及能在窄窗滚动且保留固定操作区的设置和弹窗。

- [主界面及设置三列对照](comparison-main.png)：原始实机、静态预期与最终实机的代表性对比。完整的 [18 组可点击对照](review.html) 引用本地 evidence 截图，在仓库副本中需要另行取得这些截图。
- [验收结果与边界](ACCEPTANCE.md)：功能验证、构建结果、平台/缩放、合理设计差异、已知原有问题及未验证范围。
- [现状清单与问题](01-current-state.md)、[22幅静态图与视觉规范](02-design-spec.md)、[分区实施记录](03-implementation-log.md)。
- [业务边界逐项审查](business-boundary-audit.md)：40个保护文件、58个业务函数与7个展示相关修改文件的证据。

截图与二进制在本工作树的 `evidence/ui-ux/2026-10-28/`（按仓库规则忽略，不提交到源码）。`baseline-manifest.json`、`design-gate-manifest.json`、`final-manifest.json`、`candidate-identities.json`记录时间和SHA256；每张最终实机图有同名状态JSON。`comparison-1.png`起为三列联系图，单张原尺寸PNG和HTML总览是主要审阅入口。

最终候选：`binaries/candidate-linux`、`binaries/candidate-windows-final.exe`。Windows最终候选使用隔离设置、CPU/WGPU、初始不激活方式保留在左屏供查看；创建时位置和前台ID均有实际记录。日用安装版本、文件关联与其它工作区不属于本次变更。

源码分支：`codex/ui-ux-refinement`，基线 `0cf20e3`。日用应用未替换。
