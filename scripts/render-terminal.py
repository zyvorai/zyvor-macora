#!/usr/bin/env python3
"""Render real `macora` terminal output into README images: PNG screenshots and a typing GIF.

Runs the commands against the live Mac with `redact` so no real addresses end up in the repo.
Needs Pillow (`pip install pillow`) and a built macora (default: target/release/macora).
Usage: scripts/render-terminal.py [outdir]   (default docs/ux)
"""
import os, re, subprocess, sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
BIN = os.environ.get("MACORA_BIN", str(ROOT / "target/release/macora"))
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "docs/ux"
BG, FG, CHROME = (22, 22, 29), (222, 222, 228), (44, 44, 54)
COLS = 118
FONT = ImageFont.truetype("/System/Library/Fonts/SFNSMono.ttf", 15)
CW = FONT.getlength("M")
LH = 21
PAD = 22
TITLE_H = 38

def pal(n):
    if n < 16:
        base = [(0,0,0),(205,49,49),(13,188,121),(229,229,16),(36,114,200),(188,63,188),(17,168,205),(229,229,229),
                (102,102,102),(241,76,76),(35,209,139),(245,245,67),(59,142,234),(214,112,214),(41,184,219),(255,255,255)]
        return base[n]
    if n < 232:
        n -= 16; v = [0,95,135,175,215,255]
        return (v[n // 36], v[(n // 6) % 6], v[n % 6])
    g = 8 + (n - 232) * 10
    return (g, g, g)

def spans(line):
    """Split an ANSI line into (text, color, bold) runs."""
    out, col, bold = [], FG, False
    for part in re.split(r"(\x1b\[[0-9;]*m)", line):
        m = re.fullmatch(r"\x1b\[([0-9;]*)m", part)
        if m:
            codes = [int(c) if c else 0 for c in m.group(1).split(";")]
            k = 0
            while k < len(codes):
                c = codes[k]
                if c == 0: col, bold = FG, False
                elif c == 1: bold = True
                elif c == 38 and k + 2 < len(codes) and codes[k+1] == 5: col = pal(codes[k+2]); k += 2
                elif 30 <= c <= 37: col = pal(c - 30)
                elif 90 <= c <= 97: col = pal(c - 90 + 8)
                k += 1
        elif part:
            out.append((part, col, bold))
    return out

def frame(lines, rows):
    """One Terminal-window image showing `lines` (ANSI strings) in a window `rows` tall."""
    w = int(CW * COLS) + PAD * 2
    h = TITLE_H + PAD + LH * rows + PAD
    im = Image.new("RGB", (w, h), BG)
    d = ImageDraw.Draw(im)
    d.rectangle((0, 0, w, TITLE_H), fill=CHROME)
    for i, c in enumerate([(255,95,86), (255,189,46), (39,201,63)]):
        d.ellipse((16 + i * 22, 12, 28 + i * 22, 24), fill=c)
    title = "macora — zsh"
    d.text(((w - FONT.getlength(title)) / 2, 10), title, font=FONT, fill=(150, 150, 165))
    for y, line in enumerate(lines):
        x = PAD
        for text, col, bold in spans(line):
            d.text((x, TITLE_H + PAD + y * LH), text, font=FONT, fill=col)
            if bold:
                d.text((x + 0.7, TITLE_H + PAD + y * LH), text, font=FONT, fill=col)
            x += CW * len(text)
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, w - 1, h - 1), radius=12, fill=255)
    out = Image.new("RGB", (w + 40, h + 40), (0, 0, 0))
    out.paste(im, (20, 20), mask)
    return out

def prompt(cmd):
    return f"\x1b[1;38;5;46m$\x1b[0m {cmd}"

def run(args):
    r = subprocess.run([BIN, "color", "always"] + args, capture_output=True, text=True,
                       env={**os.environ, "COLUMNS": str(COLS)})
    return r.stdout.rstrip("\n").split("\n")

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    # Screenshots
    big = run(["redact", "group", "Network"])
    frame([prompt("macora redact group Network")] + big, len(big) + 1).save(OUT / "cli-overview.png", optimize=True)
    diag = subprocess.run([BIN, "color", "always", "diagnose"], capture_output=True, text=True).stdout.rstrip("\n").split("\n")
    frame([prompt("macora diagnose")] + diag, len(diag) + 1).save(OUT / "cli-diagnose.png", optimize=True)
    # Typing GIF
    scenes = [("macora en0 redact", ["en0", "redact"], 2400), ("macora list up", ["list", "up"], 1800),
              ("macora en0 raw redact", ["en0", "raw", "redact"], 2600), ("macora diagnose", ["diagnose"], 3600)]
    rows = max(len(run(a)) for _, a, _ in scenes) + 2
    rows = max(rows, 14)
    frames, durations = [], []
    def add(lines, ms):
        frames.append(frame(lines, rows)); durations.append(ms)
    add([prompt("")], 600)
    for cmd, args, hold in scenes:
        out = subprocess.run([BIN, "color", "always"] + args, capture_output=True, text=True).stdout.rstrip("\n").split("\n")
        for n in range(0, len(cmd) + 1, 2):
            add([prompt(cmd[:n] + "\x1b[7m \x1b[0m")], 70)
        add([prompt(cmd)], 350)
        for k in range(4, len(out) + 4, 4):
            add([prompt(cmd)] + out[:k], 60)
        add([prompt(cmd)] + out + [prompt("\x1b[7m \x1b[0m")], hold)
    small = [f.resize((int(f.width * 0.8), int(f.height * 0.8)), Image.LANCZOS).convert("P", palette=Image.ADAPTIVE, colors=96) for f in frames]
    small[0].save(OUT / "macora-cli.gif", save_all=True, append_images=small[1:], duration=durations, loop=0, optimize=True, disposal=2)
    print("wrote", OUT / "cli-overview.png", OUT / "cli-diagnose.png", OUT / "macora-cli.gif", len(frames), "frames")

main()
