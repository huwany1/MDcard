# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Create a local viewer for unmodified native screenshots and step sequences."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent.parent
evidence = root / "evidence/workbench-redesign-2026-10-22"
groups = {
    "同条件分屏 · 前后": ["before-workspace/zh-CN-Light/split-1280x800.png", "after-final/zh-CN-Light/split-1280x800.png"],
    "同条件设置 · 前后": ["before-workspace/zh-CN-Light/preferences.png", "after-final/zh-CN-Light/preferences.png"],
    "同条件窄窗查找 · 前后": ["before-workspace/zh-CN-Light/compact-find-replace.png", "after-final/zh-CN-Light/compact-find-replace.png"],
}
for label, directory in [
    ("流程 · 文件夹与副稿", "master/navigation-master-r9"),
    ("流程 · 文稿保护与原生保存", "master/documents-master-r9-native"),
    ("流程 · 编码与结构撤销", "master/encoding-structure-final-r9"),
    ("流程 · 真实Pinyin组合/提交/取消", "master/ime-final-r9-verified"),
    ("组件 · 悬停/按下/错误恢复/空结果", "component-states-final-r9-menu"),
    ("边界 · 空工作区/未命名稿", "empty-final/ru-Dark"),
    ("边界 · 俄语长名dirty双窗", "dual-final-r9-ru"),
]:
    groups[label] = [path.relative_to(evidence).as_posix() for path in sorted((evidence / directory).glob("*.png"))]
for language in ["zh-CN", "zh-TW", "en", "ja", "ko", "ru"]:
    for appearance in ["Light", "Dark"]:
        directory = evidence / "after-final" / f"{language}-{appearance}"
        groups[f"最终矩阵 · {language}/{appearance}"] = [path.relative_to(evidence).as_posix() for path in sorted(directory.glob("*.png"))]
for name, paths in groups.items():
    if not paths or any(not (evidence / path).is_file() for path in paths):
        raise ValueError("Missing screenshot group: " + name)
template = """<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>MDcard · 本轮原生证据</title>
<style>body{font:15px system-ui,sans-serif;margin:0;background:#e9e9ec;color:#24252a}header{padding:16px 24px;background:#fafafa;border-bottom:1px solid #ccc}h1{font-size:20px;margin:0 0 8px}p{margin:6px 0;font-size:13px;color:#50545b}nav{display:flex;gap:10px;flex-wrap:wrap;margin:12px 0}select,button,a{font:inherit}button,select{padding:7px;border:1px solid #b9bbc2;border-radius:6px;background:white}main{padding:16px 24px}img{display:block;max-width:100%;height:auto;box-shadow:0 6px 28px #0002;border-radius:8px}#path{overflow-wrap:anywhere}a{color:#2255ba}button:focus-visible,select:focus-visible,a:focus-visible{outline:3px solid #5987ed} </style>
<header><h1>MDcard 工作台重设计 · 原生截图与流程逐帧演示</h1><p>当前Windows程序 SHA256：4e54b4eab36e42be653e040e9a00ef1c5691303b1c61d2cc12ac503715f18e3b。前后比较为同文档/尺寸/主题/物理125%DPI，原始PNG未经修图。</p><p>此页为截图序列浏览，不是连续录像。精确文本、选区、字节、进程与性能见同目录JSON。严格滚动慢帧与未验证平台见验收说明。</p>
<nav><select id="group" aria-label="证据组"></select><button id="prev">上一帧</button><select id="frame" aria-label="帧"></select><button id="next">下一帧</button><a id="raw" target="_blank">查看原图</a></nav><p id="path"></p></header><main><img id="shot" alt="MDcard原生运行截图"></main>
<script>const groups=__DATA__,g=document.querySelector('#group'),f=document.querySelector('#frame');
for(const name of Object.keys(groups)){g.add(new Option(name,name));}
function show(){const path=groups[g.value][Number(f.value)];document.querySelector('#shot').src=path;document.querySelector('#raw').href=path;document.querySelector('#path').textContent=path;}
function change(){f.replaceChildren();groups[g.value].forEach((path,i)=>f.add(new Option((i+1)+' · '+path.split('/').at(-1),i)));show();}
g.onchange=change;f.onchange=show;document.querySelector('#prev').onclick=()=>{f.selectedIndex=Math.max(0,f.selectedIndex-1);show();};document.querySelector('#next').onclick=()=>{f.selectedIndex=Math.min(f.length-1,f.selectedIndex+1);show();};change();</script></html>"""
output = evidence / "review.html"
output.write_text(template.replace("__DATA__", json.dumps(groups, ensure_ascii=False)), "utf-8")
print(json.dumps({"output": str(output), "groups": len(groups), "references": sum(map(len, groups.values())), "all_paths_exist": True}))
