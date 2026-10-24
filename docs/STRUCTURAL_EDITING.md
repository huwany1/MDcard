# 可视化表格与块操作

2026-10-19，目标范围：通过原生 GUI 或快捷键快速增删表格行列、调整列对齐；拖拽或快捷键移动段落块、受控调整多级列表。源码仍是唯一文档真值；没有引入另一份富文本文件格式。

## 操作

点击源码区标题旁的 **结构操作**，或菜单 `··· → 结构操作`，或 **Ctrl+Shift+T**。Write / Split / Read 均可打开结构窗；普通文本文件不显示 Markdown 结构功能。窗口可拖动、调整大小，表格支持横纵滚动，块列表支持纵向滚动；最小原生窗口为 620×420 逻辑像素。

**表格**页直接编辑 Markdown 单元格。下拉框选择文档内的顶层 GFM 表格；按钮操作当前单元格所在行列。右键单元格、行手柄或列手柄打开上下文菜单；列菜单支持在当前列之前插入，行菜单支持在正文行之前插入。增加行默认放在当前行之后，增加列默认放在当前列之后；表头只能修改，不能单独删除。至少保留一列；“删除表格”可整体删除，Ctrl+Z 原子恢复。

单元格显示原始 Markdown 内联写法，例如 `**强调**`、`a\|b`、`` `code` ``；预览继续使用 GFM 渲染。输入未转义的 `|` 自动加转义；尾部反斜杠用安全空格与分隔符隔开，防止吞并邻列。粘贴换行转为空格；需要多行块或复杂容器表格时使用源码。

| 操作 | 快捷键 |
| --- | --- |
| 打开 / 关闭结构窗 | Ctrl+Shift+T |
| 单元格前后导航 | Tab / Shift+Tab |
| 最后一个单元格后增加一行 | Tab；新行首格获得焦点 |
| 第一格向前导航 | Shift+Tab；保持第一格 |
| 当前行之后增加 / 删除当前正文行 | Ctrl+Alt+Enter / Ctrl+Alt+Backspace |
| 当前列之后增加 / 删除当前列 | Ctrl+Alt+→ / Ctrl+Alt+← |
| 当前列左 / 中 / 右对齐 | Ctrl+Alt+1 / 2 / 3 |
| 当前块上移 / 下移 | Alt+↑ / Alt+↓ |
| 列表项增加 / 减少层级 | Alt+→ / Alt+← |
| 撤销 / 重做 | Ctrl+Z / Ctrl+Shift+Z（也支持 Ctrl+Y） |

表格快捷键既可作用于网格焦点，也可作用于源码光标所在表格。删除末行或末列后，焦点转到合理的存活单元格；保存与取消待保存切换继续保留网格焦点和选区。返回源码按钮保持源码光标，Read 模式会切换到 Write。

**段落块**页列出顶层段落、标题、引用、整张表格、整个列表及各列表项。选择条目后使用上移、下移、增加层级、减少层级。拖动左侧六点手柄，目标出现强调色反馈线；向下放在目标之后，向上放在目标之前。远距离移动是插入重排，中间块保持原有顺序。列表项的全部后代跟随父项移动。

同级移动要求同一父项、同一列表组和同一层级。列表首项没有前一兄弟，不能增加层级；增加层级时挂到前一兄弟下。减少层级只允许当前父项下最后一个兄弟及其后代一起退出，防止后续项意外失去父项。深度、复杂空白/标记前缀超出安全范围时拒绝并在状态栏和结构窗提示。代码、HTML 块不能用结构移动；包含这些内容的完整列表子树仍按原文移动。

## 同步、保护与验收矩阵

`pulldown-cmark` 的 offset AST 确定可操作块，表格保留原始单元格/行字节范围。不把整棵 AST 重新序列化：未修改内容、转义管道、代码跨度、稀疏行、超额单元格与混合换行继续保留。增删列或修改边缘单元格时，仅在所改源行补齐必要外侧管道，防止空单元格、单列行或 Markdown 块前缀变为标题、列表、空行等其他结构。稀疏行删除实际存在或缺失的列，均保留该数据行与幸存 raw cell 的内容/空白；超额 body 单元格仍可编辑，表头超出 schema 的虚拟格禁用，须先增加列，Tab 会跳过该格。BOM/编码与原子保存由既有 `Document` 继续管理。表格嵌套在引用或列表中时保留源码编辑，避免不安全地改写容器前缀；围栏中的“伪表格”不出现于网格。

所有源编辑、单元格编辑和结构命令共用该文档的 `TextEdit` 撤销器。结构命令前后建立明确检查点；新分支丢弃旧重做。旁路只保存轻量的视图书签（源码状态指纹、逻辑块锚点、单元格位置/方向选区、原焦点所有者），不保存第二份文档文本。撤销/重做同时恢复文本和书签；打开另一文档使用独立 generation。跨块选区会拒绝结构命令，避免扩大修改范围。

解析索引按文档变化失效，未改动帧不重建 AST；表格行和块列表按视口绘制。源撤销器初始化时按文档大小设置最多 100 个检查点，目标文本快照预算 32 MiB，至少保留 2 个；很大的文档可撤回步骤会减少。该预算针对打开时的大小，后续大量粘贴增长仍沿用该文档初始策略。16 MiB 打开上限和已有源码编辑/预览的整体驻留内存边界保持不变。

| 项目 | 当前证据 |
| --- | --- |
| 管道、反斜杠、代码跨度、稀疏/超额单元格、CRLF、无末换行、2→1列 | `src/structure.rs` 单元测试，包括 0–6 连续反斜杠与实际 AST 对照、schema 2–4 × body 1–schema+1 × 四种边框的列增删矩阵、空边缘/单列保行、块前缀保护及 Unicode 方向选区 |
| 远距离块重排、全部列表后代、父项/列表组限制 | 单元测试，四块双向重排、紧凑列表边界、复杂容器保护 |
| 共用撤销与元数据 | 真正 egui Context / Editor 测试覆盖 typing→结构→typing、废弃 redo、方向选区、重复内容块、cell 末列删除 undo/redo、源码与块焦点所有者；同帧 Text/Paste→结构命令→Text、末格 Tab→Text、合帧 2–3 次相同快捷键逐条执行 |
| Linux 原生 GUI | `evidence/structure/linux-final/result.json`，UUID、二进制 SHA256、新鲜截图、真实 xdotool 鼠标键盘与逐步磁盘断言；Debian 12 / X11 Xvfb / Openbox / llvmpipe OpenGL |
| GUI 表格 | 原字节保存、Unicode/管道编辑、Tab/Shift+Tab、末格恰好增加一行、末行/末列删除后继续输入、保存后继续输入、前插列右键、对齐快捷键、整表删除 undo |
| GUI 块与列表 | 真指针远距离拖拽及目标反馈、移动后身份保持、Undo 后选中原块；源码 Alt 上下/缩进/退级及首项拒绝 |
| 六语言最小窗口 | 六种语言在 620×420 的真实 Linux 截图；目录 `linux-final/locale-*-compact.png`，翻译完整性/字体测试一并执行 |
| Windows 当前版本 | `evidence/structure/windows-final/result.json`，原生 Windows / GNU release / 125% DPI，新鲜鼠标键盘操作及磁盘断言；53/53 原生单元测试通过 |
| 主代理独立复核 | `evidence/structure/master-review/`；25/25独立核心harness、Windows53/53原生测试及另套Windows GUI（尾反斜杠、撤销后cell选区、保存焦点、redo分支、删列撤销、BOM/CRLF、新文档历史隔离）；独立 Linux `:110` 全 GUI/六语言复验，二进制哈希与实施侧一致 |

Linux release 基准包含真实 1,078,039-byte Markdown、14,001 块、100 次 cell 补丁、共用撤销事务与 AST 重建，日志中 P95 为 14.56 ms。真实 GUI 使用最终 release 二进制、1,051,036-byte 文档与 14,001 块，在没有本任务编译/打包并发时分别测量：

| 负载 | 命令 CPU P95 / 最大 | update CPU P95 / 最大 | 结果 |
| --- | --- | --- | --- |
| 100 次列对齐，180 ms 间隔 | 12.62 / 13.37 ms | 41.45 / 67.92 ms（200样本） | 100 次全执行 |
| 100 次增加行，20 ms 间隔，发送约2秒 | 13.35 / 14.21 ms | 115.39 / 115.39 ms（20样本） | 精确增加100行，无丢失 |

原始样本和 UUID 在 `linux-final/large-timing.json` 与 `result.json`。运行于 Docker 的 Intel i7-13650HX 主机、软件 OpenGL。高频 burst 的长帧来自同一帧按事件顺序处理多个完整 AST/共用撤销事务，再布局整个源码 `TextEdit`；该压力边界不保证持续60fps。以上是 eframe update CPU 时间，**不包含 GPU 提交/呈现、输入投递或显示器延迟**，不等于端到端交互延迟承诺。

主代理用另一显示 `:110` 独立重跑完整脚本：顺序命令 P95 12.36 ms、update P95 40.18 ms/最大67.65 ms；burst 100/100 无丢失、update P95 104.41 ms/最大104.97 ms。原始记录在 `master-review/linux-independent/result.json`，与上述实施侧同一最终二进制 SHA256。

复验：

```powershell
docker exec mdcard-dev cargo test --release --locked -- --nocapture
docker exec mdcard-dev cargo fmt --check
docker exec mdcard-dev cargo clippy --all-targets --locked -- -D warnings
docker exec mdcard-dev python3 scripts/verify-structure-linux.py
pwsh -NoProfile -STA -File scripts/verify-structure-windows.ps1 -Exe artifacts/structure/MDcard.exe
```

Linux GUI 脚本独占 `:109`，正常或异常退出均关闭自己的程序、Openbox、Xvfb；不要同时对同一显示执行两份脚本。Windows 脚本仅操作专属测试进程和隔离设置，需要解锁桌面、前台输入及 125% DPI；脚本拒绝没有实际前台焦点的情形。Windows MSVC、Wayland、其他桌面、所有 DPI、完整 IME 输入法行为没有在本次范围内逐一验证。
