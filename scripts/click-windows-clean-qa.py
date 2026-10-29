# SPDX-License-Identifier: AGPL-3.0-only
"""Real pointer input to this task's loopback-only QEMU VNC display."""
import argparse,socket,struct,time
p=argparse.ArgumentParser();p.add_argument('x',type=int,nargs='?',default=0);p.add_argument('y',type=int,nargs='?',default=0);p.add_argument('--text');p.add_argument('--key');p.add_argument('--release-modifiers',action='store_true');a=p.parse_args()
with socket.create_connection(('127.0.0.1',5959),timeout=3) as s:
 def read(n):
  data=b''
  while len(data)<n:
   chunk=s.recv(n-len(data))
   if not chunk:raise RuntimeError('VNC connection closed')
   data+=chunk
  return data
 version=read(12);s.sendall(b'RFB 003.008\n');security=read(read(1)[0]);assert 1 in security;s.sendall(b'\x01');assert read(4)==b'\0'*4
 s.sendall(b'\x01');header=read(24);width,height=struct.unpack('>HH',header[:4]);read(struct.unpack('>I',header[20:24])[0]);assert 0<=a.x<width and 0<=a.y<height
 def key(code,down):s.sendall(struct.pack('>BBHI',4,1 if down else 0,0,code));time.sleep(.04)
 if a.release_modifiers:
  for code in [0xffe1,0xffe2,0xffe3,0xffe4,0xffe9,0xffea,0xffeb,0xffec]:key(code,False)
 if a.text is not None:
  for char in a.text:
   code=0xff0d if char=='\n' else ord(char);key(code,True);key(code,False)
 elif a.key:
  names={'ctrl':0xffe3,'alt':0xffe9,'shift':0xffe1,'end':0xff57,'enter':0xff0d,'esc':0xff1b}
  codes=[names.get(k,ord(k) if len(k)==1 else 0) for k in a.key.split('+')];assert all(codes)
  for code in codes:key(code,True)
  for code in reversed(codes):key(code,False)
 elif not a.release_modifiers:
  s.sendall(struct.pack('>BBHH',5,0,a.x,a.y));time.sleep(.1);s.sendall(struct.pack('>BBHH',5,1,a.x,a.y));time.sleep(.1);s.sendall(struct.pack('>BBHH',5,0,a.x,a.y));time.sleep(.1)
 print('Sent guest VNC input; display',width,height)
