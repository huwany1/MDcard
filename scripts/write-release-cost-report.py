# SPDX-License-Identifier: AGPL-3.0-only
import json
from pathlib import Path
root=Path(__file__).resolve().parent.parent;e=root/'evidence/release-015'
data=json.loads((e/'final-cost-model.json').read_text(encoding='utf-8-sig'))
lines=['# MDcard 0.1.5 下载与首次使用成本','',
'本报告使用相同冻结产品源码、相同完整功能及 Node/Java/Chromium/Pandoc/字体版本。比较同一完整 payload 的 ZIP6（Windows）/gzip6（Linux）基线与真正安装包。没有将历史证据、旧二进制或不相关开发目录的清理计作压缩收益。', '',
'首先实测发现同 payload 解压仅约 4 秒，主要可节约项是数百 MB 下载。采用资源闭包、独立离线 Java image、保留必要字体和全部许可证，并在可比 payload 上比较标准 ZIP/gzip 与 LZMA2/xz。未采用首次联网补依赖、功能拆包或外部启动器。', '',
'## 实测与口径','',
'每个安装/解压及每个平台的每种 GUI 流程至少三次；原始未舍入样本在 evidence。暖 OS 页缓存，每轮全新安装/解压目录及隔离应用状态；正式阶段无编译、重压缩或运行中的虚拟机。没有声称冷磁盘或物理屏幕扫描输出。', '',
'GUI A 从无文档参数启动开始，经真实 Open 按钮/快捷键与原生文件对话框，编辑中文/英文并保存。GUI B 追加公式和三类图表的 Split 预览及原生菜单 PDF 导出。九格式另有功能验收。自动化固定输入停顿计入 GUI 数值，未将其当作人工思考时间；独立搜索验证和截图等验收动作在两场景计时区间之外。', '',
'总数按第 i 轮安装/解压 + 第 i 轮 GUI 原始数值配对，再取三轮完整和的中位数；这些阶段分别测量后合成，不冒充从浏览器点击下载开始的连续人工实测。新增恢复控制脚本保持产品 payload 逐字节不变，最终新包重新测量三次安装；GUI 复用同一 manifest/原生 SHA 的六轮实测。', '',
'| 平台 | ZIP/gzip 基线 B | 安装包 B | 少下载 B | 降幅 |', '| --- | ---: | ---: | ---: | ---: |']
for platform,v in data['platforms'].items():lines.append(f"| {platform} | {v['baselineBytes']:,} | {v['installerBytes']:,} | {v['downloadSavedBytes']:,} | {v['downloadSavedBytes']/v['baselineBytes']*100:.2f}% |")
lines+=['','应用安装及首次完整使用额外下载为 **0 B**。Linux 本轮基础镜像已有所有 Depends，故系统依赖下载亦为 0；其他目标机按 apt 方案另计，不泛化为所有 Debian/Ubuntu。官方 Windows eval ISO、QEMU、IBus、Xvfb 等仅为验收设备，不随应用安装或首用下载。','',
'| 平台/方案 | 安装或解压 s | 首次窗口/报告 s | GUI A s | GUI B 追加 s | A 安装+使用 s | B 安装+完整使用 s |','| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
for platform,v in data['platforms'].items():
 for kind,m in v['medians'].items():lines.append(f"| {platform}/{kind} | {m['preparationSeconds']:.3f} | {m['startupSeconds']:.3f} | {m['guiASeconds']:.3f} | {m['guiBAdditionalSeconds']:.3f} | {m['AWithoutNetworkSeconds']:.3f} | {m['BWithoutNetworkSeconds']:.3f} |")
lines+=['','## 固定带宽估算（不是网络实测）','','估算下载时间 = 文件字节 × 8 ÷ 固定带宽，不含服务端限速、TLS/重传、磁盘与网络重叠。正数表示新包节省，负数表示更慢。','', '| 平台 | 固定 Mbps | A 合成节省 s | B 合成节省 s |','| --- | ---: | ---: | ---: |']
for platform,v in data['platforms'].items():
 for n in v['modeledNetworkScenarios']:lines.append(f"| {platform} | {n['fixedBandwidthMbps']} | {n['AModeledSavingSeconds']:.3f} | {n['BModeledSavingSeconds']:.3f} |")
lines+=['',f"Windows A/B 盈亏带宽约 {data['platforms']['windows']['breakEvenMbps']['A']:.2f}/{data['platforms']['windows']['breakEvenMbps']['B']:.2f} Mbps。20 Mbps 场景 A/B 分别节省约 30.66/29.93 秒；100 Mbps 则分别慢约 6.93/7.66 秒。不能据包更小宣称所有网络条件更快。Linux 20 Mbps 节省约 38.71/39.85 秒，100 Mbps 仍节省约 6.79/7.93 秒。",'',
'## 必需操作与取舍','',
'按相同粒度的明确动作计算，下载链接/浏览器保存操作为双方共有，不计作差额。Windows 便携基线：打开 ZIP → Extract All → 确认提取 → 启动 exe；新包：打开 setup → 目录页 Next → 可选任务页 Next 后自动启动。准备阶段 4→3，**少 1 步**；Open→选文档→编辑→保存及完整使用的预览/导出操作双方相同。桌面/关联可选，默认不选，不计作必需收益。未知发布者/SmartScreen 的条件性额外确认未作真实浏览器下载实测，可能增加操作；不伪造信誉或人工耗时。', '',
'Linux 标准命令安装+菜单启动与 tar 解压+运行均为两项主要动作，**未证明少操作**；sudo 密码和 apt 确认可能再增加 1–2 次交互。没有把测试镜像已具备、可跳过的依赖安装步骤算成所有用户的节省。Linux 的主要收益为更少下载、包管理升级/卸载与数据恢复路径。', '',
'代价：Windows 静默安装等待约比 ZIP 解压多 15.88 秒；Linux 全文件安装后校验约比解压多 1.23 秒。安装格式自带身份、入口、升级/卸载和恢复保护，未删功能或推迟准备。Windows 已安装逻辑文件约 970.80 MB（含 manifest 与卸载器）；Linux 应用目录约 942.82 MB，标准入口/校验清单和 dpkg 元数据另计，具体 logical/allocated 实测见发行清单。', '',
'升级还会临时保存完整旧程序：Windows 旧完整目录实测约 979.53 MB，备份 29.97 秒、直接恢复 32.29 秒；真实复制期取消到退出并恢复约 28.87 秒。全新安装不备份旧目录。Linux 有效旧资源升级被 SIGINT 打断时，未发生自动完整回滚；保留 root-only 完整 SHA 备份，实测一条明确恢复命令还原全部旧文件，再标准修复重装。该人工恢复步骤不计入成功首用时间，作为失败恢复成本明确保留。', '',
'## 可复核输入','',
'原始样本：`evidence/release-015/final-{windows,linux}-protected-install`、`final-windows-gui-r3/measurements.json`、`final-linux-gui/measurements.json`、`final-linux-install/baseline-linux.json`、`baseline-windows-final-measure.log`。聚合算法：`scripts/summarize-release-costs.py`；机器可读结果：`final-cost-model.json`。所有失败/慢样本与早期试验保留，并与最终冻结采样分开标注。','']
(root/'docs/releases/0.1.5-COSTS.md').write_text('\n'.join(lines).replace('942.82 MB','942.81 MB'),encoding='utf-8')
print('Wrote docs/releases/0.1.5-COSTS.md')
