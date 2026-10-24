# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Write actual cleanup totals and a complete retained-file inventory with reasons."""
from pathlib import Path
import collections,datetime,json,os,stat
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'evidence/cleanup-icon-suite-20261024'
def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
host=read(OUT/'result.json');docker=read(OUT/'docker-result.json');before=read(OUT/'verification-before.json');after=read(OUT/'verification-after.json')
if not host['complete'] or not docker['complete'] or not before['passed'] or not after['passed'] or before['latest_sha256']!=after['latest_sha256']:raise RuntimeError('Cleanup is not verified')
plan=read(OUT/'plan.json')
if any((ROOT/row['path']).exists() for row in plan['targets']):raise RuntimeError('A cleanup target remains')
def reason(name):
 if name.startswith('.git/'):return 'Git历史','完整历史与可回溯提交，不改写历史或全局GC'
 if name.startswith('dist/windows/'):return '当前Windows运行目录','Icon Suite R3完整运行依赖与源码快照，当前桌面快捷方式目标'
 if name.startswith('dist/linux/'):return '当前Linux运行目录','正式R29完整运行依赖及源码快照'
 if name.startswith('dist/'):return '正式归档与回滚','当前分发归档/latest，或保留的最近Windows R29完整回滚ZIP'
 if name.startswith('artifacts/reading-precision/delivery/'):return '唯一历史证据归档','原始输入、失败与性能观测不能由源码重建'
 if name.startswith(('artifacts/','evidence/')):return '验收与复现证据','真实截图、原始日志、执行过的测试PE、固定夹具、哈希及独立核准；未当作可再生缓存'
 if name.startswith('assets/'):return '产品图标与字体','完整可编辑图标、字体原件/优化字体、许可证和来源；仅工具node_modules缓存被删除'
 if name.startswith('docs/'):return '有效文档与历史合同','操作/架构合同、原始独立验收、失败边界、路线图、发布身份或清理记录'
 if name.startswith(('src/','vendor/','locales/','renderer/','scripts/','tests/','examples/','.github/')):return '源码与复现工具','产品、受控依赖、锁文件、测试/脚本及示例，不是可删编译产物'
 return '项目配置与入口','Cargo/构建配置、README或许可证等必要工程文件'
rows=[];groups=collections.defaultdict(lambda:{'files':0,'bytes':0});pending=[ROOT]
while pending:
 p=pending.pop()
 with os.scandir(p) as entries:
  for e in entries:
   path=Path(e.path);s=e.stat(follow_symlinks=False);name=path.relative_to(ROOT).as_posix()
   is_link=stat.S_ISLNK(s.st_mode) or bool(getattr(s,'st_file_attributes',0)&0x400)
   if stat.S_ISDIR(s.st_mode) and not is_link:pending.append(path);continue
   cat,why=reason(name);size=0 if is_link else s.st_size
   rows.append((name,size,'链接本体，不遍历' if is_link else cat,why));groups[cat]['files']+=1;groups[cat]['bytes']+=size
inventory=OUT/'retained-files.tsv'
with inventory.open('w',encoding='utf-8',newline='') as stream:
 stream.write('path\tbytes\tcategory\treason\n')
 for name,size,category,why in sorted(rows):stream.write(f'{name}\t{size}\t{category}\t{why}\n')
 stream.write('evidence/cleanup-icon-suite-20261024/retained-files.tsv\t\t审计清单自身\t自身大小留空避免自引用；此清单反映生成时刻\n')
total_files=int(host['deleted_files'])+int(docker['deleted_files']);total_bytes=int(host['deleted_bytes'])+int(docker['deleted_bytes'])
summary={'complete':True,'created_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'workspace_files':int(host['deleted_files']),'workspace_bytes':int(host['deleted_bytes']),'duplicate_markdown_files':int(host['deleted_markdown_files']),'docker_files':docker['deleted_files'],'docker_bytes':docker['deleted_bytes'],'total_deleted_files':total_files,'total_logical_bytes':total_bytes,'retained_snapshot_files':len(rows),'retained_categories':dict(groups),'verification_before_passed':True,'verification_after_passed':True,'release_pointer_unchanged':True}
(OUT/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
gib=lambda b:f'{int(b)/2**30:.2f}'
lines=['# 图标交付后资源清理（2026-10-24）','',
 '当前 Windows 为 Icon Suite R3，Linux 为 R29；正式程序、归档及 `dist/latest.json` 在清理前后均保持原字节。桌面 MDcard 仍指向 `dist/windows/MDcard.exe`。', '',
 '## 实际清理','',
 f'工作区删除 **{int(host["deleted_files"]):,} 个文件、{int(host["deleted_bytes"]):,} 字节（{gib(host["deleted_bytes"])} GiB）**；项目 Docker 编译缓存删除 **{docker["deleted_files"]:,} 个文件、{docker["deleted_bytes"]:,} 字节（{gib(docker["deleted_bytes"])} GiB）**。合计 **{total_files:,} 个文件、{total_bytes:,} 字节（{gib(total_bytes)} GiB）**。', '',
 f'其中 **{int(host["deleted_markdown_files"]):,} 个 Markdown** 是候选完整包、重复解压及依赖目录中的副本。有效源码文档、历史验收原件和唯一证据未按日期删除；新增[文档导航](README.md)，并修正旧报告的“当前版本”措辞。', '',
 '| 清理目标 | 原因 |','| --- | --- |']
for entry in plan['targets']:lines.append(f'| `{entry["path"]}` | {entry["reason"]} |')
lines+=['','Docker 仅清空已核对挂载的 `mdcard-target` 卷 `/work/target` 内容；保留卷、容器、Rust/MinGW工具链及 `mdcard-cargo` 下载依赖。清理前无活跃编译或测试应用。', '',
 f'D盘执行窗口可用空间净增 {gib(int(host["free_after"])-int(host["free_before"]))} GiB；该读数可能受其他进程影响。删除统计是文件逻辑字节，Docker虚拟磁盘不承诺立即缩小。', '',
 '## 保留项目与理由','',
 '| 类别 | 原因 |','| --- | --- |',
 '| 当前Windows/Linux展开运行目录及对应归档 | 运行与分发用途不同，均为用户当前正式入口 |',
 '| Windows R29完整ZIP | 最近可回滚版本；其重复展开备份已经删除 |',
 '| assets/branding/mdcard-icon | 原始12 SVG、27 PNG、3 ICO、预览和可复现工具；运行时图标不是缓存 |',
 '| 原始截图、失败日志、测试PE、夹具及核准JSON | 历史观测不可由源码重新生成；保持当前验收与发布指针引用有效 |',
 '| reading-precision/delivery证据ZIP | 大型但唯一的原始历史证据归档，不属于无用构建缓存 |',
 '| 源码、Git、vendor、字体/许可证、renderer、tests、scripts | 产品、受控依赖、来源合规和复现材料 |',
 '| docs下的合同、设计、验收与发布记录 | 保留操作规范、根因、独立结论及未验证边界；导航已分类 |', '',
 '逐文件清单及原因见[retained-files.tsv](../evidence/cleanup-icon-suite-20261024/retained-files.tsv)，分类统计见[summary.json](../evidence/cleanup-icon-suite-20261024/summary.json)。清单包含Git，反映生成时刻；后续提交会增加Git对象。', '',
 '## 路径去向与验证','',
 'R1/R2旧完整包及R3重复候选已删除；其 `packages.json`、manifest、编译输入等小型身份记录按原相对路径保留在 `evidence/cleanup-icon-suite-20261024/retired-metadata/`。R3源码、资源与程序可从正式Windows完整ZIP提取；R29回滚目录可从保留的R29 Windows ZIP重新解压。历史报告中的被清理路径代表采集时位置，不将它们改指其他二进制。', '',
 '真实截图、原始失败、源稿夹具、主代理脚本、最终独立报告、`release-clearance.json` 和发布记录保留原路径。没有修改正式包内文档/manifest，也没有重新编译或重打包。', '',
 '清理前后均逐文件检查 Windows **22,257**、Linux **22,003** 个payload的精确集合、长度和SHA256，同时核对两份完整归档及EXE/ELF哈希，全部通过。证据：[清理前](../evidence/cleanup-icon-suite-20261024/verification-before.json)、[清理后](../evidence/cleanup-icon-suite-20261024/verification-after.json)。', '',
 '显式删除计划和逐项结果：[plan.json](../evidence/cleanup-icon-suite-20261024/plan.json)、[deletion-results.jsonl](../evidence/cleanup-icon-suite-20261024/deletion-results.jsonl)、[工作区结果](../evidence/cleanup-icon-suite-20261024/result.json)、[编译卷结果](../evidence/cleanup-icon-suite-20261024/docker-result.json)。未扫描或删除真实稿件、用户配置、其他项目、系统临时目录或Docker全局资源。','']
(ROOT/'docs/RESOURCE_CLEANUP_ICON_SUITE_2026-10-24.md').write_text('\n'.join(lines),encoding='utf-8',newline='\n')
print(json.dumps({k:summary[k] for k in ['total_deleted_files','total_logical_bytes','duplicate_markdown_files','retained_snapshot_files']},indent=2))
