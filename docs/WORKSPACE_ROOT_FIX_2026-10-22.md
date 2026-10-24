# 文件夹工作区根目录修复（2026-10-22）

打开 `.github/workflows/build.yml` 等多层子目录文件时，侧栏此前会从原工作区切换为文件的父目录，原有祖先、同级目录及展开状态随之消失。原因是每次成功打开文件都通过 `sync_file_root` 无条件采用当前文件的父目录。

现在工作区内的打开、编码重读、另存为均保留工作区根目录；新建及欢迎文档也保留工作区。成功打开或另存为工作区外的文件才改用新的父目录。命令行或文件关联启动时仍以传入文件的父目录建立初始工作区；打开失败保留原工作区，启动文件打开失败时回到启动目录。Windows 启动目录先规范为与文档一致的 canonical 路径，避免普通盘符路径与 `\\?\` 路径比较不一致。

两处检索提示及六种语言翻译已同步为工作区语义。扫描预算、后台刷新合并、文件类型策略、保存和未保存提示保护继续采用原流程。

## 验证

- 新增回归测试在修改业务逻辑前失败，证实根目录从临时工作区变成 `.github/workflows`；同一测试修复后通过。
- 真实临时文件及编辑器动作覆盖多层目录、中文及空格目录、展开状态、同级文件、指定编码重读、新建及欢迎文档；覆盖未保存导航请求及 Escape 取消、失败打开、相同文字前缀的外部相邻目录和原文件字节保护。
- Debian 12 构建容器执行 `cargo test --locked`：117 通过，0 失败，3 个既有独立性能/布局测试保持 ignored；`cargo clippy --locked --all-targets -- -D warnings`、`cargo fmt --all -- --check` 均通过。
- Windows 原生窗口，实际系统 DPI 120（125%），运行修复后的 GNU release 程序：点击展开 `.github/workflows` 并打开 `build.yml` 后，根目录及支持文件数保持不变，当前文件高亮、祖先及同级目录保留；随后通过同级中文目录点击、刷新、Ctrl+P、创建新文档、命令行启动及无参数启动验证，三份输入文件 SHA256 均未改变，三次窗口正常退出。

Windows 自动化命令：

```powershell
pwsh -NoProfile -STA -File scripts/verify-workspace-root-windows.ps1 -Exe dist/windows/MDcard.exe -Stage dist
```

鼠标移动到控件后等待180ms，再发送按下/松开，避免自动化输入先于窗口处理鼠标移动；剪贴板步骤要求 STA。早期输入定位和原生文件选择器脚本失败记录保留，不作为产品失败或最终通过记录。外部文件重定位及未保存取消的最终完整覆盖由编辑器回归测试提供；本次未进行 Linux 原生窗口验收。

证据目录：`evidence/workspace-root-2026-10-22/`。修改前失败日志为 `regression-before.log`，修复后为 `regression-after.log`，最终完整测试及 Clippy 日志为 `core-tests-final.log`、`clippy-final.log`；最终 Windows 窗口记录为 `windows-tree-final-2/result.json`（UUID `d14ffc0a-72ce-4946-baf4-48413c266a6b`）及同目录六组截图/状态。

Windows 程序 SHA256：`c9c6981712f054578587d4a1fe978330eaec0920b5372eee6e4032a85473af3a`。当前修复交付入口为 `dist/windows/MDcard.exe` 和 `dist/MDcard-0.1.1-workspace-root-20261022-windows-x64.zip`；完整 manifest 及包哈希见 `dist/latest.json`。Linux 分发程序保持原有 R11 版本。
