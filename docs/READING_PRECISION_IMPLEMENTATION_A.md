# 阶段 A 实现证据快照

本文件只汇总阶段 A 的实现侧证据，不能替代独立 Windows GUI 验收或阶段 B 门槛。完整验收编号、失败历史和主代理结论见 [READING_PRECISION_ACCEPTANCE.md](READING_PRECISION_ACCEPTANCE.md)，行为和预算见 [READING_PRECISION.md](READING_PRECISION.md)。

## 源码与构建身份

R10 冻结于基准 HEAD `a40985806058efcf25f92fe888a30abc68c0f773` 上的本阶段产品变更；阶段提交由主代理协调。`artifacts/reading-precision/stage-a-r10-source.json` 记录并重新核对全部 28 个产品文件，聚合摘要 `5c0096fa3c1de55f9d55e972aa0b4ed353562659ee2dbcdd7f82412263461f75`。

| 构建 | 路径 | SHA256 |
| --- | --- | --- |
| Windows GNU 原生应用 | `artifacts/reading-precision/stage-a-r10-windows/MDcard.exe` | `3dd8edc357f79edf28ffa377c9baa657f5f7040bc90e32b530c485a15b414374` |
| Linux 原生应用 | `artifacts/reading-precision/stage-a-r14-linux/mdcard` | `cfa218d0eafd5748b16ae55417fa6eb2ae2df285257ded11e50c0a422864e148` |
| Windows 原生测试程序 | `artifacts/reading-precision/stage-a-r10-native/mdcard-tests.exe` | `822889a7693847ea18545364621b25e54a25bd1745b388c2f38f36bdd18cb97e` |

轻量汇总 `artifacts/reading-precision/stage-a-r10-implementation-final.json` 绑定上述产品哈希、原生测试 manifest 和下列十组 Linux 最终结果。原始 JSON、截图、输入稿及失败候选留在忽略目录，不将巨量诊断加入 Git。

## 实现与自动化

- 保存解码原稿的扩展字素簇范围和有限上下文，独立于 cursor/selection；恢复使用实际绘制 Galley 的源映射和目标 glyph 几何。实体代表字素与完整 token 范围分开，虚拟窗口切断的字素不冒充完整文字。
- 宽度、字体、模式和异步高度变化保留同一 pin；当前高度变化导致 glyph 暂不绘制时重新挂载同一源块，不反复使用失效 target。逻辑行、视觉行、块级降级和首尾/横向夹取分别报告。
- 非文字恢复期间保护原块记录，使用真正 TexturePoll 等待图片布局；不会用临时顶部文字覆盖图片中的历史。有限步数/时间预算失败保留原记录并停止争抢滚动。
- Linux 根测试 153 通过、3 忽略；Windows 本机执行测试程序 146 通过、2 忽略。根 fmt、Linux/Windows strict clippy 均通过。另有故障、search 和 performance 独立 crate 日志；它们包含重用的模块测试，计数不相加。
- 字体重排后 glyph 被高度重测卸载的回归使用真正 headless 绘制和原生 RawInput；它在撤掉该修复时失败，修复后通过。headless 测试只算自动化，不代替 GUI。

对应日志均为 `artifacts/reading-precision/stage-a-r10-{core,fmt,clippy,windows-clippy,faults,search,performance}.log`；Windows 原生 stdout/manifest 在 `stage-a-r10-native/`。

## 当前构建的 Linux GUI

平台为 Debian 12、X11、Xvfb、Openbox、Mesa 软件 OpenGL，原生鼠标键盘输入，读取当前实际 Galley 几何并保存截图。范围不扩大到其他桌面、显示服务器或 GPU。每次精确重排继续观察 2.1 秒，补偿计数必须停止。

| 最终运行目录（`evidence/reading-precision/implementation/` 下） | 实际覆盖和结果 |
| --- | --- |
| `stage-a-r10h-rich-font` | Read 四次宽窄、字体 15→23、Split 往返；Write 两宽、字体 23→14、Split 往返。原稿 glyph 不变，观测 y 误差 0 px，尾段稳定。 |
| `stage-a-r10b-mega-middle` | 原单段范围 `[35,1394035)`；真实输入推进 Read byte 671911（48.1977%）、Write byte 683358（49.0189%），再宽窄/分屏。稳定目标分别 672075/683500；最大重排 y 误差 1.25 px，尾段不持续补偿。 |
| `stage-a-r10-mega-late-txt` | 原单逻辑长行中真实推进 byte 1190118（85.3718%），WrapOn 后宽窄/分屏，稳定 pin 1190260；最大重排 y 误差 1.25 px，尾段稳定。 |
| `stage-a-r10-async` | 真实 loopback PNG 延迟释放，offset 400→1475；真实 KaTeX 高度变化，offset 399→619。同原 glyph 最大 y 误差分别 1/0 px，2.1 秒补偿停止；补偿计数分别 29/2，不隐去加载期间的多次补偿。 |
| `stage-a-r10-boundaries` | 3200 px 高图片中的非文字位置 1600 正常重启，每轴误差 0，未复活旧文字 pin。 |
| `stage-a-r10-cancellation` | 实际 pending 中投递 XTest wheel；只读事件 `raw_wheel=true/had_pending=true`，从观测 pending 到注入 0.000694 ms，原生输入到取消可见上界 50.305015 ms；请求采样 1 ms，实测最大间隔 15.460649 ms，随后 2 秒无拉回。仅代表布局恢复取消。 |
| `stage-a-r10-retained` | 14 项原生切换/正常重启/编码/普通与大文档等回归，位置比较各轴误差 0。映射类别精确验收另由独立 GUI/自动化完成，不能把此处块级往返计为新文字映射测量。 |
| `stage-a-r10-{ordinary,one-mib,ten-mib}-performance` | 三档相同原生输入，双独立进程交错写入、重启恢复、真正 OS flock 写入失败/内存保留/旧磁盘不变/解锁重试均通过。原稿哈希不变。 |

历史失败均保留：包括早期量化误差循环、非文字图片首次布局保护、源码/预览虚拟窗口未挂载 pin、失效精确 target，以及驱动未触发字体 Slider、轮滚落入画布、读取到诊断写入中间状态等。最终成功目录不替代这些原始失败记录。

## 实测函数墙钟与边界

输入文档约 53 KiB、1 MiB、10 MiB，含中英、emoji、重复内容和长段；同机相同 native wheel 序列。每档 100 次向下输入及后续独立文档交错/向上输入，诊断 JSON 输出启用。`Instant.elapsed` 是函数墙钟，不能称线程 CPU、显示时延或 FPS。

| 档位 | capture+plan+submit 热帧 P95 / max（ms） | 样本数 | GUI 主段 P95（ms） | 元数据索引墙钟（ms，1 次） | 观测 app RSS 峰值（两进程，MiB） |
| --- | --- | --- | --- | --- | --- |
| 普通 | 0.059445 / 2.037626 | 356 | 0.686783 | 1.391199 | 358.785 |
| 1 MiB | 0.055393 / 0.327455 | 356 | 1.092423 | 2.497669 | 421.797 |
| 10 MiB | 0.055997 / 0.085369 | 317 | 5.230070 | 10.294521 | 1105.254 |

热路径达到事先冻结的 P95≤2 ms、单次≤8 ms。索引、布局、主线程提交和落盘另有原始 P50/P95/max/n；大文档索引在后台，不并入主线程热帧。

**GUI 主段不是完整 update。** 旧 `whole_update_scroll_cpu_ms` / `rich.scrollCpuMs` 名称保留以兼容原始结果，实际不含入口前置任务及尾部清理/诊断输出，不能据此宣布完整 update 增量预算通过。阶段 B 将使用实施前 `264d42d` 的隔离归档，仅加相同入口到返回观测，和最终 A+B 同机同输入比较；R10 配对只能说明 B 增量，不能取代实施前基线。冻结的 20%/2 ms 门槛不变。

100 次向下输入开始到持久化确认共约 7.53 秒（包含约 1 秒停止后等待），分别观测成功写入 3/2/3 次，不能说这些写入全部发生于持续输入期间。后台落盘事务 max 分别 77.670/25.654/25.026 ms。50 ms 外部采样保留实际成功写入时间和 app PID RSS；RSS 不是私有分配器峰值，也不含 renderer 子孙进程。真实诊断文件写入与阅读状态持久化写次数分别计算。

## 保留项

| 项目 | 阶段 A 状态 |
| --- | --- |
| 支持路径的同原文字和实际几何、明确夹取、失败保护 | 本构建已有上述自动化/GUI 证据；最终独立全矩阵由主代理判定。 |
| 完整 App::update 对实施前功能基线的总增量预算 | 未验证，阶段 B 以相同观测闭合，不能用主段结果代替。 |
| 元数据索引计算真正中止、新文档不等待旧索引/匹配 | 未完成，阶段 A 只有后台计算和 epoch 拒绝迟到结果；真实布局取消不等于计算已经中止。 |
| 大幅内容变化后精确重定位、1000+200+200 独立集门槛、Stage B GUI | 后续阶段 B，尚未实现/通过；现产品对改变内容的纯 API 不声明文字 exact。 |
| 未实测桌面/GPU/UNC 共享、任意稿件精准保证 | 未验证；本轮集合结论不能扩成普遍保证。 |
| 最终阶段提交、完整包/资源 manifest 与包内 GUI | 主代理在独立验收后完成并给出对应身份，此快照不提前声明。 |
