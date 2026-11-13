#!/usr/bin/env python3
"""生成 1024x1024 应用图标（纯标准库，无第三方依赖）。

设计：圆角方块 + 青绿径向渐变 + 白色圆环 + 白色上箭头（提）。
运行：python3 scripts/gen-icon.py app-icon.png
"""

import math
import struct
import sys
import zlib

SIZE = 1024
SS = 2  # 超采样倍数（每个方向）


def sd_round_box(px, py, cx, cy, hw, hh, r):
    qx = abs(px - cx) - (hw - r)
    qy = abs(py - cy) - (hh - r)
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    inside = min(max(qx, qy), 0.0)
    return outside + inside - r


def sd_segment(px, py, ax, ay, bx, by):
    pax, pay = px - ax, py - ay
    bax, bay = bx - ax, by - ay
    denom = bax * bax + bay * bay
    h = 0.0 if denom == 0 else max(0.0, min(1.0, (pax * bax + pay * bay) / denom))
    return math.hypot(pax - bax * h, pay - bay * h)


def coverage(sdf, edge=1.0):
    return max(0.0, min(1.0, 0.5 - sdf / edge))


def lerp(a, b, t):
    return a + (b - a) * t


def gradient(px, py):
    # 径向渐变：左上偏亮的薄荷绿 -> 深墨绿
    cx, cy = SIZE * 0.34, SIZE * 0.28
    d = math.hypot(px - cx, py - cy) / (SIZE * 0.9)
    t = min(1.0, max(0.0, d))
    t = t ** 0.85
    top = (74, 222, 159)   # #4ade9f
    bottom = (4, 120, 87)  # #047857
    return (
        lerp(top[0], bottom[0], t),
        lerp(top[1], bottom[1], t),
        lerp(top[2], bottom[2], t),
    )


def write_png(path, width, height, pixels):
    raw = b"".join(
        b"\x00" + pixels[y * width * 4 : (y + 1) * width * 4] for y in range(height)
    )
    compressed = zlib.compress(raw, 9)

    def chunk(tag, data):
        body = tag + data
        return (
            struct.pack(">I", len(data))
            + body
            + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", compressed)
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as fh:
        fh.write(png)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "app-icon.png"
    buf = bytearray(SIZE * SIZE * 4)

    half = SIZE / 2
    radius = SIZE * 0.225  # 圆角
    inset = SIZE * 0.035
    hw = hh = half - inset

    ring_r = SIZE * 0.315
    ring_w = SIZE * 0.062

    arrow_w = SIZE * 0.052
    tip_y = half - SIZE * 0.135
    shoulder_y = half + SIZE * 0.035
    wing_x = SIZE * 0.145

    for y in range(SIZE):
        row = y * SIZE * 4
        for x in range(SIZE):
            px = x + 0.5
            py = y + 0.5

            # 圆角方块底
            box = sd_round_box(px, py, half, half, hw, hh, radius)
            a = coverage(box)
            if a <= 0:
                continue

            r, g, b = gradient(px, py)

            # 白色圆环
            ring_sdf = abs(math.hypot(px - half, py - half) - ring_r) - ring_w / 2
            ring = coverage(ring_sdf) * 0.96

            # 白色向上箭头（两段）
            left = sd_segment(px, py, half - wing_x, shoulder_y, half, tip_y) - arrow_w / 2
            right = sd_segment(px, py, half, tip_y, half + wing_x, shoulder_y) - arrow_w / 2
            arrow = max(coverage(left), coverage(right)) * 0.97

            fg = max(ring, arrow)
            # 顶部高光
            if fg > 0:
                r = lerp(r, 255, fg)
                g = lerp(g, 255, fg)
                b = lerp(b, 255, fg)

            idx = row + x * 4
            buf[idx] = int(r)
            buf[idx + 1] = int(g)
            buf[idx + 2] = int(b)
            buf[idx + 3] = int(a * 255)

    write_png(out, SIZE, SIZE, bytes(buf))
    print(f"wrote {out} ({SIZE}x{SIZE})")


if __name__ == "__main__":
    main()
