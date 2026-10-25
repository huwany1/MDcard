# MDcard 0.1.0

Windows x64 与 Linux x64 均来自同一冻结源码 [c453689171b09463c57cc217fa48974a080854f2](https://github.com/huwany1/MDcard/tree/c453689171b09463c57cc217fa48974a080854f2)，应用、包名和 manifest 统一为 0.1.0。

正式应用包、SHA256SUMS.txt 与发行元数据见[本次 Release](https://github.com/huwany1/MDcard/releases/tag/v0.1.0)。

- 保留最新编辑、源码同步、结构编辑、搜索替换与阅读状态功能，包含公开主线的路径别名重命名、ELK 隔离和 xmldom 修复。
- Windows 在 OpenGL 初始化失败且尚未创建编辑器时自动使用 CPU 图形兼容模式，保留 GPU 偏好；PE、命令行和设置可见版本一致。
- 全新发行目录仅含应用、所需动态库和本地引擎、实际引用资源、许可证、精简说明和逐文件完整性清单。
- 使用固定来源的 unbranded ungoogled-chromium（Windows 155.0.8059.84 / Linux 156.0.8078.47）、裁剪 Java 21 模块及已锁定 Node/Pandoc；两平台引擎版本与独立哈希均如实记录。
- 支持本地公式、Mermaid、PlantUML、flowchart 和 HTML/PDF/PNG/JPEG/DOCX/EPUB/Marp HTML/PDF/PPTX 导出。远程图片及外链需网络。

Windows 11 x64；Linux Debian 12 x64 / glibc 2.36、X11，依赖清单见 README。其他系统/桌面/驱动、macOS 未全面验证。当前没有自动保存或崩溃恢复保证。

归档文件、哈希与保留理由由发行元数据和包内 manifest 绑定；[最终 C01—C07 验收报告](https://github.com/huwany1/MDcard/blob/main/docs/releases/0.1.0-acceptance.json)独立记录真实远程删除、上传和公开下载结果，冻结时的元数据不冒充发布后状态。历史标签和发布记录继续保留。

自有代码与新应用为 AGPL-3.0-only；第三方运行时保留各自许可证和声明；历史 MIT 权利不撤销。
