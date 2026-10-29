# SPDX-License-Identifier: AGPL-3.0-only
import argparse,socket,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('command',nargs='*');p.add_argument('--screenshot',type=Path);p.add_argument('--type');p.add_argument('--port',type=int,default=4559);a=p.parse_args()
with socket.create_connection(('127.0.0.1',a.port),timeout=3) as s:
    s.settimeout(.3)
    try:s.recv(32768)
    except TimeoutError:pass
    commands=[' '.join(a.command)] if a.command else ['info status']
    if a.type:
        mapping={':':'shift-semicolon','\\':'backslash','/':'slash','.':'dot',' ':'spc','\n':'ret','-':'minus','_':'shift-minus','%':'shift-5','|':'shift-backslash','(':'shift-9',')':'shift-0','"':'shift-apostrophe',"'":'apostrophe','=':'equal',',':'comma','$':'shift-4','[':'bracket_left',']':'bracket_right','{':'shift-bracket_left','}':'shift-bracket_right',';':'semicolon','+':'shift-equal','*':'shift-8','&':'shift-7'}
        for char in a.type:
            key=mapping.get(char,('shift-'+char.lower()) if char.isupper() else char);s.sendall(('sendkey '+key+' 20\n').encode());time.sleep(.06)
            try:s.recv(65536)
            except TimeoutError:pass
    if a.screenshot:
        a.screenshot.parent.mkdir(parents=True,exist_ok=True);ppm=a.screenshot.with_suffix('.ppm')
        commands.append('screendump "'+str(ppm.resolve()).replace('\\','/')+'"')
    for command in commands:
        s.sendall((command+'\n').encode());time.sleep(.4)
        try:
            response=s.recv(65536).decode(errors='replace')
            print('\n'.join(line for line in response.splitlines() if '\x1b' not in line and line.strip()!='(qemu)'))
        except TimeoutError:pass
if a.screenshot:
    from PIL import Image
    Image.open(ppm).save(a.screenshot);print(a.screenshot)
