#!/usr/bin/env python3
"""开发自测辅助：通过 XTEST 模拟鼠标移动 / 长按 / 单击。

用法:
  python3 scripts/xinput.py move <x> <y>
  python3 scripts/xinput.py hold <x> <y> <毫秒>
  python3 scripts/xinput.py click <x> <y>
"""

import ctypes
import sys
import time

X11 = ctypes.CDLL("libX11.so.6")
XTST = ctypes.CDLL("libXtst.so.6")

X11.XOpenDisplay.restype = ctypes.c_void_p
X11.XOpenDisplay.argtypes = [ctypes.c_char_p]
X11.XFlush.argtypes = [ctypes.c_void_p]
XTST.XTestFakeMotionEvent.argtypes = [
    ctypes.c_void_p,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_ulong,
]
XTST.XTestFakeButtonEvent.argtypes = [
    ctypes.c_void_p,
    ctypes.c_uint,
    ctypes.c_int,
    ctypes.c_ulong,
]


def open_display():
    display = X11.XOpenDisplay(None)
    if not display:
        raise SystemExit("无法打开 X display")
    return display


def move(display, x, y):
    XTST.XTestFakeMotionEvent(display, -1, int(x), int(y), 0)
    X11.XFlush(display)


def button(display, down):
    XTST.XTestFakeButtonEvent(display, 1, 1 if down else 0, 0)
    X11.XFlush(display)


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    display = open_display()
    command = sys.argv[1]

    if command == "move":
        move(display, sys.argv[2], sys.argv[3])
    elif command == "hold":
        move(display, sys.argv[2], sys.argv[3])
        time.sleep(0.2)
        button(display, True)
        time.sleep(int(sys.argv[4]) / 1000.0)
        button(display, False)
    elif command == "click":
        move(display, sys.argv[2], sys.argv[3])
        time.sleep(0.15)
        button(display, True)
        time.sleep(0.08)
        button(display, False)
    elif command == "drag":
        # 从 (x0,y0) 拖到 (x1,y1)，用于测试拖窗与边缘吸附
        x0, y0, x1, y1 = (int(v) for v in sys.argv[2:6])
        move(display, x0, y0)
        time.sleep(0.2)
        button(display, True)
        time.sleep(0.25)
        steps = 14
        for i in range(1, steps + 1):
            move(display, x0 + (x1 - x0) * i // steps, y0 + (y1 - y0) * i // steps)
            time.sleep(0.035)
        time.sleep(0.15)
        button(display, False)
    elif command == "wheel":
        # 向下滚动：按钮 5 按 N 次
        move(display, sys.argv[2], sys.argv[3])
        time.sleep(0.15)
        for _ in range(int(sys.argv[4])):
            XTST.XTestFakeButtonEvent(display, 5, 1, 0)
            XTST.XTestFakeButtonEvent(display, 5, 0, 0)
            X11.XFlush(display)
            time.sleep(0.05)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
