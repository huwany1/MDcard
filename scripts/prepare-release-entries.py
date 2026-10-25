# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Prepare reviewable unavailable/final public entries without changing a repo.

Regenerate after the final release.json/SHA256SUMS freeze. The plan binds exact
input hashes, so candidate metadata cannot accidentally become the final index.
"""
from pathlib import Path
import argparse,datetime,hashlib,json,re
p=argparse.ArgumentParser();p.add_argument('--source-root',type=Path,required=True);p.add_argument('--release-index',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=a.source_root.resolve();index_path=a.release_index.resolve();base=index_path.parent;out=a.output.resolve()
if out.exists():raise RuntimeError('Use a fresh public-entry preparation directory')
index=json.loads(index_path.read_text('utf8'));assert index['version']=='0.1.0';source=index['sourceCommit'];release='https://github.com/huwany1/MDcard/releases/tag/v0.1.0';downloads='https://github.com/huwany1/MDcard/releases/download/v0.1.0/';source_url='https://github.com/huwany1/MDcard/tree/'+source;report='https://github.com/huwany1/MDcard/blob/main/docs/releases/0.1.0-acceptance.json'
def sha(path):
 with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
names=['MDcard-0.1.0-windows-x64.zip','MDcard-0.1.0-linux-x64.tar.gz','SHA256SUMS.txt','MDcard-0.1.0-release.json'];assets=[{'name':name,'bytes':(base/name).stat().st_size,'sha256':sha(base/name)} for name in names]
packages={item['platform']:item for item in index['packages']};original=(root/'README.md').read_text('utf-8-sig');old_install=original[original.index('## 安装与运行'):original.index('## 已有能力')]
libraries='libx11-6 libxcb1 libxkbcommon0 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libgl1 libegl1 libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libatspi2.0-0 libcups2 libdrm2 libdbus-1-3 libgbm1 libasound2 libpango-1.0-0 libcairo2 libfontconfig1 libfreetype6 xdg-desktop-portal xdg-desktop-portal-gtk'
def write(stage,name,text):
 path=stage/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf8',newline='\n')
def scrub(value):
 if isinstance(value,dict):return {key:scrub(item) for key,item in value.items()}
 if isinstance(value,list):return [scrub(item) for item in value]
 if isinstance(value,str) and '/releases/download/' in value and re.search(r'MDcard.*\.(zip|tar\.gz)|SHA256SUMS',value,re.I):return None
 return value
for phase in ['unavailable','final']:
 stage=out/phase;available=phase=='final'
 title='[下载正式包]('+release+')' if available else '发行更新与校验进行中，应用下载暂停'
 intro='两平台采用同一冻结源码构建，完整解压后即可运行，保留 `renderer`、许可证和 `manifest.json`。无需另装 Node、Java、Chromium、Pandoc 或开发工具。'
 links='\n'.join(f"- {platform.title()} x64：[{item['archive']}]({downloads+item['archive']})，{item['bytes']:,} 字节（{item['bytes']/1048576:.2f} MiB）。" for platform,item in packages.items()) if available else 'Windows / Linux 0.1.0 正在完成发行准备和公开下载闭环，完成前不提供应用压缩包下载。'
 integrity='[SHA256SUMS.txt]('+downloads+'SHA256SUMS.txt) · [发行元数据]('+downloads+'MDcard-0.1.0-release.json)' if available else '校验资产与下载入口将在完整上传和下载复验后恢复；请勿使用历史应用包的校验值验证新包。'
 install=f'''## 安装与运行

{links}

{intro}

Windows 11 x64：运行 `MDcard.exe`。默认 OpenGL 不可用时自动使用 Windows 内置 CPU 图形后端，保留原 GPU 偏好；程序版本可用 `MDcard.exe --version` 查询。Windows 10 未作本轮支持声明。

Linux x64：Debian 12 / glibc 2.36、X11 桌面，运行 `./mdcard`。系统需桌面图形/字体/音频库和可工作的原生文件对话框 portal；这些是系统桌面前提，应用运行时已随包提供。Debian 12 依赖：

```sh
sudo apt-get install {libraries}
```

{integrity.rstrip("。")}。两平台真实构建源为 [{source[:12]}]({source_url})，自有应用版本统一 0.1.0。逐文件白名单与用途、大小和 SHA256 位于包内 `manifest.json`；完整验收边界见[本次验收报告]({report})和[正式发行索引](docs/releases/latest.json)。

本包包含路径别名重命名、ELK 隔离、xmldom 修复与最新已交付功能；本地公式图表和全部九种导出格式使用随包引擎。用户稿件、设置、源码快照、开发与测试工具、品牌交接资料和历史报告不随应用包分发。项目自有源码与资产统一采用 AGPL-3.0-only；第三方内容见[来源与例外清单](docs/THIRD_PARTY.md)。

'''
 readme=original.replace('# MDcard\n','# MDcard 0.1.0\n',1).replace('[下载正式包](https://github.com/huwany1/MDcard/releases/tag/v0.1.1-20261024)',title,1).replace('[0.1.0 发行准备页](https://github.com/huwany1/MDcard/releases/tag/v0.1.0)',title,1).replace(old_install,install)
 write(stage,'README.md',readme)
 navigation=(root/'docs/README.md').read_text('utf-8-sig')
 navigation_line=('当前 Windows / Linux 正式版本统一为 **0.1.0**。' if available else 'Windows / Linux 统一 0.1.0 发行准备和下载校验进行中，应用下载暂停。')+'运行和下载入口以[项目 README](../README.md)与[正式发行索引](releases/latest.json)为准。历史报告中的版本、路径和测试结果仅代表采集时状态，旧应用包不再作为当前下载入口。'
 navigation=re.sub(r'^当前 Windows 正式版本.*$',navigation_line,navigation,flags=re.M)
 write(stage,'docs/README.md',navigation)
 search=(root/'docs/GLOBAL_SEARCH.md').read_text('utf-8-sig')
 search=search.replace('当前保留的发行包是 `dist/MDcard-0.1.1-windows-x64.zip` 与 `dist/MDcard-0.1.1-linux-x64.tar.gz` 完整包。','下文所述 0.1.1 完整包也是历史交付身份；当前统一 0.1.0 发行状态和下载入口见[项目 README](../README.md)与[正式索引](releases/latest.json)。')
 write(stage,'docs/GLOBAL_SEARCH.md',search)
 notes=f'''# MDcard 0.1.0

Windows x64 与 Linux x64 均来自同一冻结源码 [{source}]({source_url})，应用、包名和 manifest 统一为 0.1.0。

{'发行资产更新和校验进行中，应用下载暂停。' if not available else '正式应用包、SHA256SUMS.txt 与发行元数据见[本次 Release]('+release+')。'}

- 保留最新编辑、源码同步、结构编辑、搜索替换与阅读状态功能，包含公开主线的路径别名重命名、ELK 隔离和 xmldom 修复。
- Windows 在 OpenGL 初始化失败且尚未创建编辑器时自动使用 CPU 图形兼容模式，保留 GPU 偏好；PE、命令行和设置可见版本一致。
- 全新发行目录仅含应用、所需动态库和本地引擎、实际引用资源、许可证、精简说明和逐文件完整性清单。
- 使用固定来源的 unbranded ungoogled-chromium（Windows 155.0.8059.84 / Linux 156.0.8078.47）、裁剪 Java 21 模块及已锁定 Node/Pandoc；两平台引擎版本与独立哈希均如实记录。
- 支持本地公式、Mermaid、PlantUML、flowchart 和 HTML/PDF/PNG/JPEG/DOCX/EPUB/Marp HTML/PDF/PPTX 导出。远程图片及外链需网络。

Windows 11 x64；Linux Debian 12 x64 / glibc 2.36、X11，依赖清单见 README。其他系统/桌面/驱动、macOS 未全面验证。当前没有自动保存或崩溃恢复保证。

归档文件、哈希与保留理由由发行元数据和包内 manifest 绑定；[最终 C01—C07 验收报告]({report})独立记录真实远程删除、上传和公开下载结果，冻结时的元数据不冒充发布后状态。历史标签和发布记录继续保留。

项目自有源码、文档、示例、网站、品牌设计及其他自有资产统一采用 AGPL-3.0-only，不再提供 MIT 备选授权；授权范围见 [COPYRIGHT](COPYRIGHT) 和[统一授权说明](docs/LICENSING.md)。第三方源码和资产继续采用其原许可，见[第三方来源与例外清单](docs/THIRD_PARTY.md)。
'''
 write(stage,'RELEASE_NOTES.md',notes)
 write(stage,'SHA256SUMS.txt',(base/'SHA256SUMS.txt').read_text('utf8') if available else '# MDcard 0.1.0 发行资产更新与校验进行中，应用下载暂停。最终校验清单在完整上传后恢复。\n')
 formal={'schema':1,'product':'MDcard','version':'0.1.0','sourceCommit':source,'sourceURL':source_url,'availability':'available' if available else 'updating','releaseURL':release if available else None,'acceptanceReport':report,'preparedFromReleaseMetadataSHA256':sha(index_path),'localAcceptancePassed':index.get('localAcceptancePassed',False),'assets':[{**item,'url':downloads+item['name'] if available else None} for item in assets],'packages':[{**item,'downloadURL':item.get('downloadURL') if available else None} for item in index['packages']]}
 write(stage,'docs/releases/0.1.0.json',json.dumps(formal,ensure_ascii=False,indent=2)+'\n');write(stage,'docs/releases/latest.json',json.dumps({'version':'0.1.0','index':'0.1.0.json','availability':formal['availability'],'releaseURL':formal['releaseURL'],'sourceCommit':source},ensure_ascii=False,indent=2)+'\n')
 for old in (root/'docs/releases').glob('*.json'):
  if old.name in ['0.1.0.json','latest.json']:continue
  historical=scrub(json.loads(old.read_text('utf-8-sig')));historical.update(current=False,availability='superseded',supersededBy='0.1.0',supersedingReleaseURL=release if available else None)
  write(stage,'docs/releases/'+old.name,json.dumps(historical,ensure_ascii=False,indent=2)+'\n')
plan={'preparedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceCommit':source,'inputReleaseMetadataSHA256':sha(index_path),'inputAssets':assets,'localAcceptancePassedAtPreparation':index.get('localAcceptancePassed',False),'mustRegenerateAfterHashFreeze':True,'remoteMutationPerformed':False,'note':'Reviewable files only. Apply unavailable phase after local acceptance; apply final phase only after actual remote upload and public verification. Regenerate against the frozen four assets first.'}
write(out,'plan.json',json.dumps(plan,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'output':str(out),'sourceCommit':source,'phases':['unavailable','final'],'remoteMutationPerformed':False}))
