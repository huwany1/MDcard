# 跨文件阅读状态验收（2026-10-23）

> 首阶段历史快照：下文“当前”及“后续阶段”指2026-10-23采集时范围。现行R29已分别完成文字/行级恢复和大幅修改后重定位，正式入口及独立限制见 [精度交付](READING_PRECISION_DELIVERY.md)；历史展开路径清理后可从完整证据ZIP按同名成员提取，见 [本轮清理记录](RESOURCE_CLEANUP_2026-10-24.md)。原36项及原始结论保留，不追认旧构建具备后续能力。

首阶段功能验收通过。交付范围是文件间独立记忆、重启恢复、重排后的同段落/内容块恢复与可靠失败降级；文字/行级恢复、大幅修改后的精确重定位及命中率报告仍为后续阶段。原始 36 个验收项保留在下表，不以源码检查代替真实 GUI 结果。

## 当前构建与证据身份

| 项目 | 身份 |
| --- | --- |
| Windows R10 | `artifacts/reading-state/windows-r10/MDcard.exe` |
| Windows SHA256 | `3027e411140bd02b3d3f1e26a1b7b9be0cdfc7e94da1ad56e146ca5469f4a146` |
| Linux R10 | `artifacts/reading-state/linux-r11/mdcard` |
| Linux SHA256 | `90c5a763bb04fdec1b096e2202741f69377b7bdb2efc746509b989540b96fb23` |
| Windows 测试程序 | `windows-r10/mdcard-native-tests.exe`，SHA256 `6d776949eab9d045a0ea7d5274de59e85c9162167fb81c7a481a81e71c712f1a` |
| 平台 | Windows 11 x64 build 26100，125% DPI；Debian 12 / Docker / X11 / Xvfb / Openbox / Mesa 软件 OpenGL |
| 完整快照 | `reading-state-final-20261023`，完整运行目录为 `dist/reading-state-final-20261023/{windows,linux}` |

`artifacts/reading-state/native-tests-r10.json` 保存构建时全部 src/Cargo 哈希；最终 Git 提交与逐文件包清单以该快照的 `manifest.json` 和 `packages.json` 为准。`dist/latest.json` 指向新版本目录，旧的 `dist/windows`、`dist/linux` 保留。原稿、阅读状态、原稿恢复及替换历史各自独立。

## 证据索引与方法

- 主代理 Windows：18 组场景、64 项 GUI 断言通过。最终采用 `master-final-r10*` 中每个完整通过的结果，`same-file` 使用 `master-final-r10b-same-file`、`priorities` 使用 `master-final-r10d-priorities`、`crash` 使用 `master-final-r10e-crash`；原始失败和旧计时记录保留。汇总与证据哈希：`evidence/reading-state/master/final-review/summary.json`、`evidence-manifest.json`。
- Linux 当前 GUI：`evidence/reading-state/implementation/linux-r11/result.json`，14 项通过；四次宽窄往返、普通/大文档、TXT、UTF-16/GB18030、转换节点、长段落、空文档及重启。其余当前结果为 `linux-async-r10`、`linux-concurrency-r10`、`linux-large-performance-r10`，均绑定上述 Linux 哈希。主代理另核对原始 JSON 和图片/公式前后实图。
- 核心：Linux 141 通过、3 忽略；Windows 测试 EXE 在本机实际运行，134 通过、2 忽略、退出 0，stderr 为空。日志为 `artifacts/reading-state/implementation-tests-r10.log`、`implementation-native-tests-r10-stdout.log`；fmt 与严格 Clippy 通过。
- 主代理独立编写的 11 项契约用例通过：`master-independent-r10.log`，过滤条件为 `independent::`。搜索 69 通过/3 忽略、性能回归 68 通过，见 `implementation-search-r10.log`、`implementation-performance-r10.log`。各 crate 可能引入同一源模块测试，计数不能相加为互不重复用例。
- GUI 输入使用真实 Win32 鼠标、键盘、原生文件对话框，或 X11/xdotool；JSON 是只读观察，不向应用注入位置。每次测试使用独立状态与设置文件；校验所有夹具字节，内容变化/重命名场景仅允许其明示的受控改动。恢复就绪同时要求 pending 完成、记录 viewport 与实际 viewport 一致，防止把过期位置当作当前结果。
- 完整包逐文件/归档成员/安全路径/哈希与 Linux 执行位校验：`evidence/reading-state/master/final-review/packages.json`；包内运行复验另见同目录记录。

## 六项需求逐项状态

下表“通过”限定为已列平台和本轮夹具；额外未验证组合在末节保留。逻辑像素容差预先约定为每轴 ≤2 px，实际误差见汇总和各场景原始 JSON。

| 编号 | 验收项 | 状态 | 当前证据或边界 |
| --- | --- | --- | --- |
| 1.1 | A 后部返回前部后重开停在最后位置 | 通过 | Windows core/semantics 与 Linux GUI 实际后退再 A→B→A，恢复最后位置。 |
| 1.2 | 当前位置不被最远位置覆盖 | 通过 | 同场景验证 `furthest` 保留较大值而恢复使用最后当前位置。 |
| 1.3 | 底部不自动已读；比例语义明确 | 通过 | semantics 连续滚轮抵达真实末端并再滚确认夹取，`read=false`；当前 UI 不显示百分比，诊断标签为“当前位置”。 |
| 2.1 | A/B、不同目录同名文件隔离 | 通过 | core/routes 原生打开根目录与 nested/same.md，分别滚动并往返恢复。 |
| 2.2 | 相对/绝对路径按规范化身份关联 | 通过 | core 用绝对路径建立记录，正常退出后以相对启动路径恢复同 ID 和位置；独立规范化用例。 |
| 2.3 | DocumentId 不被打开代次替代 | 通过 | 普通重开、宽度/渲染重建稳定 ID；代次只约束控件和异步结果。独立 late-generation 回归。 |
| 2.4 | 未命名草稿之间独立 | 通过 | guards 真实新建/放弃后 ID 不同及核心用例；不承诺草稿正文跨重启恢复。 |
| 2.5 | 阅读、源码、光标和选区分别恢复 | 通过 | core/source 普通 Markdown、422 KiB Markdown、普通/455 KiB TXT，文首选区与后部横纵滚动分别跨文件/重启恢复。 |
| 2.6 | 主副窗格、阅读/源码/分屏隔离 | 通过 | secondary/reflow/source；same-file 还验证同文件 ID、相同 revision=1、不同正文/BufferId 的索引与 UTF-8 边界独立，以及副窗格 worker 保存后的继续记录。 |
| 2.7 | 内部重命名迁移；另存为复制且保留原记录 | 通过 | rename 真实原生 Save As 新 ID 继承位置，原文件保留；菜单改名后 ID、位置与原稿字节跨重启保持。 |
| 3.1 | 内容/布局不变，恢复 xy 并报告实际误差 | 通过 | core/source/formats 与 Linux GUI 的切换、重启实测；含 x>0 的普通/大源码视图。汇总最大误差与样本数另列。 |
| 3.2 | 宽度、字号、换行变化保持同段/块 | 通过 | core/reflow/formats 的宽窄、字号滑块、换行开关、分屏；TXT 按解码后原始逻辑段落边界判定。 |
| 3.3 | 前文变更按上下文/标题定位，重复内容不盲选 | 通过 | content 实际加入中文/emoji 前缀，原段落 byte 随 UTF-8 前缀长度移动；重复上下文、标题歧义及 >257 候选截断保守降级有针对性回归。 |
| 3.4 | 锚点删除或不可靠时降级，安全且不称精确 | 通过 | content 将原稿替换为短文，实际 `approximate`/`nearby-heading`/`empty` 退化、边界夹取，无崩溃；歧义核心用例。 |
| 3.5 | 无记录、空、缩短、末尾安全 | 通过 | 新同名文档起顶、空文档 GUI、短文降级与真实末端场景；末尾受可滚范围夹取，不承诺末字恢复。 |
| 3.6 | 普通/大文档/纯文本、中文 emoji、混合编码 | 通过 | formats 八类实测与 Linux GUI；UTF-16、GB18030 使用解码后 Unicode 坐标，编码字节不作为预览坐标。 |
| 3.7 | 原 Markdown 与转换后预览映射 | 通过 | 普通/大 mapping.md 真实 GUI；针对性 math/mermaid/Unicode 原源码映射断言，生成 token 偏移与原稿偏移不同。 |
| 3.8 | 后续精确定位命中率、误定位率和失败案例 | 后续阶段 | 没有大幅修改后精确定位的完成声明；文字/行级精确锚点及其专项统计另立阶段。 |
| 4.1 | 文件树、原生打开、Recent、Quick、搜索一致 | 通过 | core/routes 五种真实入口，同一成功提交路径；命令行相对/绝对启动也实测。原生拖放路由已接入，拖放手势未单独复测。 |
| 4.2 | 取消、保存失败、目标失败时旧稿及位置可用 | 通过 | guards 原生对话框取消、Quick 取消、坏 UTF-16 打开失败、脏稿取消；真实 Windows 文件句柄拒绝原稿保存，正文和位置保留。 |
| 4.3 | 快速 A→B→C，迟到 B 不拉动 C | 通过 | priorities 真实快速树点击，确认 B 曾成为中间文件，C 后 1.5 秒身份/偏移稳定；显式延迟 worker 核心用例单独证明迟到任务丢弃。 |
| 4.4 | 解析/布局未就绪不以顶部覆盖历史 | 通过 | 普通/大文档重启 GUI；后台元数据按文档+buffer+epoch/revision 校验，完成帧更新 layout/viewport，布局就绪前不捕获。 |
| 4.5 | 搜索、目录优先历史且不被拉回 | 通过 | routes 实际搜索选择原始 Unicode 源码命中，TOC 点击并等待尾段，历史不覆盖目标。 |
| 4.6 | 主动滚动取消待完成恢复 | 通过 | priorities 重开 8,446,660 B 历史大文档，独立观察线程捕获目标 ID/打开代次对应 pending，1.0799 ms 后投递实际 Win32 滚轮；历史 1359.9998 px 与用户 1839.9996 px 不同，pending 清空且 1.5 秒尾段稳定。metadata 未就绪时的输入另有核心回归。 |
| 5.1 | 宽窄、侧栏、字号、换行、分屏保持块 | 通过 | core/reflow/formats 和 Linux 连续四次宽窄往返，均比较原段落/映射源块。 |
| 5.2 | 上方异步图片/公式增高得到补偿且不漂移 | 通过 | async-r10 真实 PNG 400→1476 px、KaTeX 400→620 px；源段落 byte 1314/937 保持，最后 1.5 秒漂移 0 px，原稿 hash 不变。 |
| 5.3 | 长段落/高块误差边界 | 通过 | 43 KiB 长段落 GUI 与高图片/公式；只要求同原段/内容块，块内比例近似，不要求同字或同视觉行。 |
| 5.4 | 首阶段只声明块级能力 | 通过 | 合同、诊断 precision 与本文一致；光标/选区恢复不等于阅读视口精确到文字/行。 |
| 6.1 | 正常重启各文件最近成功落盘位置 | 通过 | core/source/formats 及 Linux 两独立进程分别重启，分别比较文件和视图位置。 |
| 6.2 | 异常退出恢复最近成功记录，报告年龄 | 通过 | crash 实际终止请求时，磁盘位置记录年龄 2163 ms，最近成功写入完成年龄 1654 ms；随后重启按原磁盘记录恢复，不承诺零丢失。 |
| 6.3 | 非每帧写盘/全文扫描，记录写频和开销 | 通过 | 普通/大文档各 100 次真实滚轮约 7.53 秒，各 2 次成功阅读状态写；索引按 buffer/revision 缓存，较大文档后台构建。实测见下节。 |
| 6.4 | 写失败保留内存、提示可查看、原稿安全 | 通过 | failure 无效父路径时 A/B 内存位置可回；设置错误详情可见，原稿字节未改。Linux 真 flock 阻塞后释放重试。 |
| 6.5 | 缺失、损坏、越界、不兼容安全降级 | 通过 | corrupt 原生 GUI 五组：坏 JSON、version999、负值、巨大值、缺失状态，均安全从顶开始；字段校验核心用例。 |
| 6.6 | 原子替换失败不破坏上一份有效记录 | 通过 | atomic 真实 Win32 FileShare 句柄拒绝替换，上一有效状态 SHA 不变；内存保留，释放后重试、重启恢复新位置。 |
| 6.7 | 当前/全部清除、容量淘汰、活动保护 | 通过 | clear 实际设置操作、再开和重启；capacity 预置 512 条后实际打开主副文档，淘汰旧记录、≤512 条/4 MiB、两活动记录保留。 |
| 6.8 | 多实例按文档合并、不整表覆盖 | 通过 | Linux 两独立 MDcard PID 共用状态 A/B/A 交错写、各自重启与真实 OS 锁重试；Windows 本机锁/合并核心通过，Windows 双 GUI 进程并发未单独复测。 |

## 误差、性能与资源边界

Windows 42 次偏移对照（包含切换、重启与稳定尾段），偏移 API 数值的最大误差为 x=0、y≈0.0000104 逻辑 px，低于预先约定的每轴 2 px 容差。该微小数值是浮点坐标差；截图验证原段/块，不据此宣称物理屏幕的亚像素精度。Linux 对应切换与重启样本两轴误差均为 0。

函数采样使用 Rust `Instant.elapsed` 的墙钟区间，非线程 CPU 计数器。原 JSON 中 `*_cpu_ms` 是兼容字段名；调度与函数区间内的诊断 I/O 开销可能计入。位置捕获不含渲染和源码坐标转换；提交计入脏记录复制和入队，磁盘事务在后台单列。开启的诊断 JSON 写不计作产品阅读状态写。这些数字不代表物理显示时延、GPU 完成时间或通用帧率上限。

| 普通文档 `linux-concurrency-r10` | 样本 | P95 | 最大/单次值 |
| --- | --- | --- | --- |
| 连续滚动 | 100 次原生滚轮，7.530220 s | 不适用 | 成功阅读状态写 2 次 |
| 位置捕获函数 | 307 | 0.007302 ms | 0.625647 ms |
| 复制+入队调用 | 4 | 0.032174 ms | 0.032174 ms |
| 整个 GUI update 函数 | 121 | 1.683356 ms | 1.990204 ms |
| 索引构建 | 1 | 不适用 | 2.719127 ms |
| 最后一次后台落盘 | 1 | 不适用 | 12.088749 ms |

| 大文档 `linux-large-performance-r10` | 样本 | P95 | 最大/单次值 |
| --- | --- | --- | --- |
| 连续滚动 | 100 次原生滚轮，7.530004 s | 不适用 | 成功阅读状态写 2 次 |
| 位置捕获函数 | 323 | 0.006511 ms | 0.066941 ms |
| 复制+入队调用 | 5 | 0.027362 ms | 0.027362 ms |
| 整个 GUI update 函数 | 114 | 2.015861 ms | 2.614218 ms |
| 后台索引构建 | 1 | 不适用 | 3.703055 ms |
| 最后一次后台落盘 | 1 | 不适用 | 11.722065 ms |

大文档实测是 422,333 B Markdown、455,284 B TXT；其余夹具最短为空、长段落 43,085 B，均包含中文/emoji 等实际文本。4 MiB/512 条是阅读状态预算，不是编辑器或 renderer 的总内存上限。首阶段重排只验同段/块；长块内比例会改变看到的具体文字，不以块内插值宣称同字恢复。

## 历史失败、后续阶段与未验证范围

历史失败原样保留：早期选区方向、完成帧过期 viewport 导致恢复循环、overlay 被误认为正文输入而复填清除状态、Save As 新控件从顶开始、虚拟源码屏外选区 clamp、重复匹配截断误判唯一、同文件双 buffer 索引共享，均推动修复并有回归。最后一项在 R9 被主代理真实 GUI 复现，在 R10 同文档 ID/revision=1 的双 buffer 场景验证隔离。

测试驱动失败也保留并区分：输入法/Clipboard、选错同名 Quick 条目、误点保存按钮、过快移动后点击未实际触发、普通 resize 恢复太快而未观测到 pending、清理夹具误假设一次 CtrlZ 必定改变正文。最终使用自然大文档加载等待、独立只读观察线程和真实轮滚验证取消；未向诊断报告注入状态，也未给产品加入测试延迟。修正后用同一 R10 二进制重新跑完整场景。

后续阶段：大幅修改后的精确重定位及命中率/误定位率/失败案例，文字/行级精确锚点。首阶段不提供草稿正文跨重启恢复，也不识别外部移动后的文件身份。未验证：Wayland、真实 GNOME/KDE、macOS、多 DPI/多显示器、中文 IME/无障碍完整链路、Windows 双 GUI 进程并发、原生文件拖放手势。当前结论限定于所列系统、夹具和块级位置。
