#!/usr/bin/env python3
"""Exercise the actual TUI in a disposable pseudo-terminal, including tty cleanup.
Python standard library only. Works on macOS/Linux; no network or mutations.
"""
import errno
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("MACORA_TEST_BINARY", ROOT / "target/debug/macora"))
FIXTURE = ROOT / "tests/fixtures/macos.ifconfig"

class TerminalTests(unittest.TestCase):
    def launch(self, rows=30, cols=110):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        saved = termios.tcgetattr(slave)
        try:
            process = subprocess.Popen([str(BINARY), "tui", "--from", str(FIXTURE), "--interval", "0.2", "--no-color"], stdin=slave, stdout=slave, stderr=slave)
        except Exception:
            os.close(master); os.close(slave)
            raise
        self.addCleanup(self.cleanup, process, master, slave, saved)
        initial = self.read_until(master, b"MACORA", 3)
        self.assertIn(b"MACORA", initial)
        return process, master, slave, saved, initial
    def cleanup(self, process, master, slave, saved):
        if process.poll() is None:
            process.kill(); process.wait(timeout=3)
        termios.tcsetattr(slave, termios.TCSANOW, saved)
        os.close(master); os.close(slave)
    def read_until(self, master, expected, timeout):
        output = bytearray(); end = time.monotonic() + timeout
        while time.monotonic() < end:
            ready, _, _ = select.select([master], [], [], 0.05)
            if ready:
                try: data = os.read(master, 65536)
                except OSError as exc:
                    if exc.errno == errno.EIO: break
                    raise
                output.extend(data)
                if expected in output: break
        return bytes(output)
    def finish(self, process, master, slave, saved, key):
        os.write(master, key)
        output = self.read_until(master, b"\x1b[?1049l", 3)
        self.assertIn(b"\x1b[?1049l", output)
        self.assertEqual(process.wait(timeout=3), 0)
        self.assertEqual(termios.tcgetattr(slave), saved)
    def test_quit_restores_original_terminal(self):
        p,m,s,before,_ = self.launch(); self.finish(p,m,s,before,b"q")
    def test_ctrl_c_restores_original_terminal(self):
        p,m,s,before,_ = self.launch(); self.finish(p,m,s,before,b"\x03")
    def test_small_terminal_does_not_panic(self):
        p,m,s,before,_ = self.launch(12,42); self.finish(p,m,s,before,b"q")
    def test_arrows_palette_pause_and_manual_refresh(self):
        p,m,s,before,_ = self.launch()
        os.write(m,b"2\x1b[B\r")
        self.assertIn(b"Detail",self.read_until(m,b"Detail",3))
        os.write(m,b":topology\r")
        self.assertIn(b"Topology",self.read_until(m,b"Topology",3))
        os.write(m,b"p")
        self.assertIn(b"PAUSED",self.read_until(m,b"PAUSED",3))
        os.write(m,b"r")
        self.assertIn(b"Capture refreshed",self.read_until(m,b"Capture refreshed",3))
        self.finish(p,m,s,before,b"q")

if __name__ == "__main__": unittest.main()
