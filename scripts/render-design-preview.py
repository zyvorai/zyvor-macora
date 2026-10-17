#!/usr/bin/env python3
"""Optional Pillow documentation renderer. Native previews are illustrations,
not SwiftUI screenshots. tui-preview renders actual Rust-renderer fixture output.
Runtime app and build do not depend on this script or Pillow.
"""
from PIL import Image,ImageDraw,ImageFont
from pathlib import Path
import math,shutil
ROOT=Path(__file__).resolve().parents[1]
S=2; W=1600; H=1190
FONTS=Path('/usr/share/fonts/truetype/dejavu')
def render(dark):
 p={'page':'#000000' if dark else '#ffffff','surface':'#1d1d1f' if dark else '#ffffff','side':'#111113' if dark else '#fafafa','text':'#f5f5f7' if dark else '#1d1d1f','muted':'#a1a1a6' if dark else '#6e6e73','line':'#38383b' if dark else '#dedee3','blue':'#2997ff' if dark else '#0071e3','select':'#162b40' if dark else '#eaf4ff'}
 im=Image.new('RGB',(W*S,H*S),'#111216' if dark else '#eef0f4');d=ImageDraw.Draw(im)
 def f(n,b=False,mono=False):return ImageFont.truetype(str(FONTS/('DejaVuSansMono.ttf' if mono else 'DejaVuSans-Bold.ttf' if b else 'DejaVuSans.ttf')),int(n*S))
 def text(x,y,s,n=13,c=None,b=False,mono=False):d.text((x*S,y*S),s,font=f(n,b,mono),fill=c or p['text'])
 def rr(box,fill,r=16,outline=None):d.rounded_rectangle(tuple(int(v*S) for v in box),r*S,fill=fill,outline=outline,width=S)
 def line(x1,y1,x2,y2,c=None):d.line((x1*S,y1*S,x2*S,y2*S),fill=c or p['line'],width=S)
 def dot(x,y,c,r=4):d.ellipse(((x-r)*S,(y-r)*S,(x+r)*S,(y+r)*S),fill=c)
 text(38,23,'MACORA 0.2',25,b=True)
 text(280,36,'RUST + NATIVE SWIFTUI  /  MAC ONLY  /  APACHE 2.0',10,c=p['muted'])
 rr((1195,25,1560,57),p['surface'],16,outline=p['line']);text(1212,35,'DESIGN ILLUSTRATION · SIMULATED VALUES',10,c=p['muted'])
 rr((30,88,1570,1160),p['page'],24,outline=p['line'])
 rr((31,89,258,1159),p['side'],24);d.rectangle((235*S,89*S,258*S,1159*S),fill=p['side']);line(258,89,258,1159)
 for x,c in [(55,'#ff6158'),(77,'#febd2f'),(99,'#28ca42')]:dot(x,111,c,6)
 rr((52,152,85,185),'#0071e3',10);text(58,153,'≈',22,c='white',b=True);text(98,152,'MACORA',14,b=True);text(99,173,'ZYVOR · FOR MAC',8,c=p['muted'])
 y=214
 for group,items in [('NETWORK',['Overview','Interfaces','Wi-Fi','Services']),('EXPLORE',['Topology','Routes','DNS','Neighbors','Connections','Network Tools']),('INTELLIGENCE',['Diagnostics','Compare','Change History'])]:
  text(52,y,group,9,c=p['muted'],b=True);y+=32
  for item in items:
   if item=='Overview':rr((44,y-7,245,y+25),p['select'],9)
   dot(60,y+8,p['blue'] if item=='Overview' else p['muted'],3)
   text(77,y,item,12,c=p['text'] if item=='Overview' else p['muted'],b=item=='Overview');y+=36
  y+=20
 rr((45,1081,245,1144),p['surface'],13,outline=p['line']);text(59,1095,'This Mac',12,b=True);text(59,1118,'Local-first · no cloud',10,c=p['muted'])
 text(285,110,'Overview',18,b=True)
 rr((1110,105,1306,139),p['side'],15,outline=p['line']);text(1125,116,'Search actions       ⌘K',10,c=p['muted'])
 text(1330,112,'◐',18,c=p['muted']);rr((1375,104,1539,140),'#0071e3',18);text(1415,115,'Configure',12,c='white',b=True)
 line(259,154,1568,154)
 text(296,189,'MACORA · NETWORK WORKSPACE',10,c=p['blue'],b=True)
 text(295,218,'Your network. In full view.',37,b=True)
 text(297,273,'Understand the connection. Explore the evidence.',15,c=p['muted'])
 text(297,298,'Make the next change with confidence.',15,c=p['muted'])
 rr((296,342,507,387),'#0071e3',22);text(318,356,'Explore Your Network',12,c='white',b=True);text(530,357,'Run diagnostics ›',12,c=p['blue'])
 text(1429,357,'On this Mac',10,c=p['muted'])
 rr((296,415,1536,721),p['surface'],24,outline=p['line']);text(320,437,'Your connection stack',20,b=True);text(1391,448,'Local collection',10,c=p['muted'])
 cards=[('Adapters','Every interface, clearly grouped.','↔'),('Wireless','Nearby networks and signal.','≈'),('Control','Services, DNS and locations.','≡'),('Evidence','Baseline before. Compare after.','▣')]
 for i,(title,detail,icon) in enumerate(cards):
  x=320+i*300;rr((x,486,x+280,628),p['surface'],16,outline=p['line']);text(x+18,500,icon,24,c=p['blue']);text(x+18,537,title,14,b=True);text(x+18,564,detail,10,c=p['muted']);text(x+18,599,'Open ›',10,c=p['blue'])
 line(320,649,1510,649)
 for x,value,label in [(322,'1','Active adapters'),(622,'8.4 MiB/s','Download'),(922,'724 KiB/s','Upload'),(1222,'0','Baseline changes')]:text(x,670,value,21,b=True);text(x,701,label,10,c=p['muted'])
 rr((296,743,1085,945),p['surface'],20,outline=p['line']);text(320,766,'Your connections',17,b=True)
 text(322,813,'Wi-Fi',15,b=True);text(397,817,'en0 · Pinned',10,c=p['muted'],mono=True);dot(1040,820,'#30b661',4)
 text(322,847,'192.168.1.8/24',13,mono=True);text(322,881,'Link active',10,c=p['muted']);text(975,881,'MTU 1500',10,c=p['muted'])
 rr((1105,743,1536,945),p['surface'],20,outline=p['line']);text(1128,767,'Workspace evidence',17,b=True)
 for n,(label,value) in enumerate([('Interface inventory','Captured'),('VPN ownership','Not inferred'),('Internet reachability','Not tested')]):
  yy=811+n*34;text(1128,yy,label,11,c=p['muted']);text(1407,yy,value,11,b=True)
 text(1128,919,'Open Network Tools ›',10,c=p['blue'])
 rr((296,968,1536,1120),p['surface'],20,outline=p['line']);text(320,990,'Traffic, in real time',13,b=True);text(1287,993,'Download    Upload',10,c=p['muted'])
 pts=[]
 for j in range(200):pts.append((321+j*5.95,1084-16*math.sin(j/13)-24*math.exp(-((j-110)/22)**2)))
 for a,b in zip(pts,pts[1:]):d.line((int(a[0]*S),int(a[1]*S),int(b[0]*S),int(b[1]*S)),fill=p['blue'],width=2*S)
 text(298,1137,'Design illustration · simulated values · not a native SwiftUI screenshot',9,c=p['muted'])
 out=ROOT/'docs'/('design-preview-dark.png' if dark else 'design-preview-light.png');im.resize((W,H),Image.Resampling.LANCZOS).save(out)
 return out
light=render(False);dark=render(True);shutil.copyfile(dark,ROOT/'docs/design-preview.png')
# Render real terminal output with no invented traffic values.
text=(ROOT/'docs/tui-fixture.txt').read_text().replace('\r','')
mono=ImageFont.truetype(str(FONTS/'DejaVuSansMono.ttf'),15)
canvas=Image.new('RGB',(1220,690),'#0b111b');draw=ImageDraw.Draw(canvas)
draw.text((28,20),'MACORA TUI · ACTUAL RUST RENDERER / SANITIZED OFFLINE FIXTURE',font=mono,fill='#38a5ff')
for n,line in enumerate(text.splitlines()):draw.text((28,58+n*19),line,font=mono,fill='#c7d6e9')
canvas.save(ROOT/'docs/tui-preview.png')
print(light);print(dark);print(ROOT/'docs/tui-preview.png')
