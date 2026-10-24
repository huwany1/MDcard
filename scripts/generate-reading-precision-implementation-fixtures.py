# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Implementation performance inputs; no independent oracle or master cases."""
import argparse
import hashlib
import json
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
args.output.mkdir(parents=True,exist_ok=True)
words='中文 English e\u0301 👩🏽‍💻 🇨🇳 ☀️ independent reading text and source coordinates. '
files={}
for name,target in [('ordinary',40*1024),('one-mib',1024*1024),('ten-mib',10*1024*1024)]:
    parts=[f'# {name} performance document\n\n'];size=len(parts[0].encode());index=0
    while size<target:
        if index%17==0:part=f'## Chapter {index:06d}\n\n'
        elif index%23==0:part=f'Long paragraph {index:06d} '+words*200+'\n\n'
        elif index%3==0:part='Repeated paragraph '+words*3+'\n\n'
        else:part=f'Paragraph {index:06d} '+words*3+'\n\n'
        parts.append(part);size+=len(part.encode());index+=1
    value=''.join(parts)
    for suffix in ['.md','.txt']:
        path=args.output/(name+suffix);path.write_text(value,encoding='utf8',newline='')
        raw=path.read_bytes();files[path.name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
(args.output/'inputs.json').write_text(json.dumps(files,indent=2)+'\n',encoding='utf8')
print(json.dumps(files,indent=2))
