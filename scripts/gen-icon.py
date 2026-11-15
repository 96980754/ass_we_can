#!/usr/bin/env python3
"""生成 1024x1024 应用图标（numpy 矢量化渲染，无第三方图像库依赖）。

图案：小蜜桃（屁股）简笔画线稿 —— 与浮窗素材同源的贝塞尔曲线，
暖桃粉色渐变圆角底 + 深色线稿。
运行：python3 scripts/gen-icon.py app-icon.png
"""

import struct
import sys
import zlib

import numpy as np

SIZE = 1024  # 输出尺寸
VB = 128.0   # 素材 viewBox 坐标系
SCALE = SIZE / VB
STROKE = 4.6  # viewBox 单位（比素材 4 略粗，缩小后更清晰）

# 与 peach_rest.svg 同源的曲线（viewBox 128）：外轮廓 + 中缝
OUTLINE = [
    ((64, 36), (49, 22), (27, 27), (19, 47)),
    ((19, 47), (10, 69), (20, 92), (40, 97)),
    ((40, 97), (50, 100), (58, 96), (64, 89)),
    ((64, 89), (70, 96), (78, 100), (88, 97)),
    ((88, 97), (108, 92), (118, 69), (109, 47)),
    ((109, 47), (101, 27), (79, 22), (64, 36)),
]
CREASE = [((64, 37), (58, 48), (58, 62), (62, 72))]


def bezier(p0, p1, p2, p3, n=40):
    t = np.linspace(0, 1, n)[:, None]
    pts = (
        (1 - t) ** 3 * np.array(p0)
        + 3 * (1 - t) ** 2 * t * np.array(p1)
        + 3 * (1 - t) * t**2 * np.array(p2)
        + t**3 * np.array(p3)
    )
    return pts


def stroke_distance(field_shape, curves):
    """所有曲线折线化的有向距离场（负=在线条内部）。"""
    dist = np.full(field_shape, 1e9, dtype=np.float32)
    for seg in curves:
        pts = bezier(*seg) * SCALE
        a = pts[:-1]
        b = pts[1:]
        for (x0, y0), (x1, y1) in zip(a, b):
            pad = STROKE * SCALE / 2 + SIZE / 64
            xlo = max(0, int(min(x0, x1) - pad))
            xhi = min(SIZE, int(max(x0, x1) + pad) + 1)
            ylo = max(0, int(min(y0, y1) - pad))
            yhi = min(SIZE, int(max(y0, y1) + pad) + 1)
            if xlo >= xhi or ylo >= yhi:
                continue
            ys, xs = np.mgrid[ylo:yhi, xlo:xhi].astype(np.float32)
            ax, ay = x0, y0
            bx, by = x1, y1
            dx, dy = bx - ax, by - ay
            denom = dx * dx + dy * dy
            if denom < 1e-9:
                d = np.hypot(xs - ax, ys - ay)
            else:
                t = np.clip(((xs - ax) * dx + (ys - ay) * dy) / denom, 0, 1)
                d = np.hypot(xs - ax - t * dx, ys - ay - t * dy)
            np.minimum(dist[ylo:yhi, xlo:xhi], d, out=dist[ylo:yhi, xlo:xhi])
    return dist


def sd_round_box(x, y, cx, cy, hw, hh, r):
    qx = np.abs(x - cx) - (hw - r)
    qy = np.abs(y - cy) - (hh - r)
    outside = np.hypot(np.maximum(qx, 0), np.maximum(qy, 0))
    return outside + np.minimum(np.maximum(qx, qy), 0) - r


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "app-icon.png"

    # 2x2 超采样网格
    off = np.array([0.25, 0.75])
    ys, xs = np.mgrid[0:SIZE, 0:SIZE].astype(np.float32)
    xs = (xs[..., None] + off[None, None, :]) - 0.5
    ys = (ys[..., None] + off[None, None, :]) - 0.5
    xs = xs[..., None] + off[None, None, None, :]
    ys = ys[..., None] + off[None, None, None, :]
    xs = xs.reshape(SIZE, SIZE, 4)
    ys = ys.reshape(SIZE, SIZE, 4)

    # 底：暖桃粉渐变圆角方块
    half = SIZE / 2
    inset = SIZE * 0.03
    box = sd_round_box(xs, ys, half, half, half - inset, half - inset, SIZE * 0.225)
    alpha = np.clip(0.5 - box, 0, 1).mean(axis=-1)

    t = np.clip(
        ((xs - half) / SIZE + (ys - half) / SIZE + 1) / 2, 0, 1
    ).mean(axis=-1) ** 0.9
    top = np.array([255.0, 205.0, 182.0])   # #ffcdb6
    bottom = np.array([234.0, 90.0, 51.0])  # #ea5a33
    rgb = top[None, None, :] + (bottom - top)[None, None, :] * t[..., None]

    # 左上高光
    hl = np.exp(-(((xs - half * 0.62) / (SIZE * 0.34)) ** 2 + ((ys - half * 0.5) / (SIZE * 0.30)) ** 2)).mean(axis=-1)
    rgb += (np.array([255.0, 255.0, 255.0])[None, None, :] - rgb) * (hl * 0.18)[..., None]

    # 线稿：深暖黑
    ink_dist = np.minimum(
        stroke_distance((SIZE, SIZE), OUTLINE),
        stroke_distance((SIZE, SIZE), CREASE),
    )
    ink = np.clip(0.5 - (ink_dist - STROKE * SCALE / 2), 0, 1)
    ink_rgb = np.array([36.0, 29.0, 24.0])  # #241d18
    rgb = rgb * (1 - ink[..., None]) + ink_rgb[None, None, :] * ink[..., None]

    buf = np.dstack([np.clip(rgb, 0, 255), alpha[..., None] * 255]).astype(np.uint8)
    write_png(out, buf)
    print(f"wrote {out} ({SIZE}x{SIZE})")


def write_png(path, arr):
    h, w = arr.shape[:2]
    raw = b"".join(
        b"\x00" + arr[y].tobytes() for y in range(h)
    )

    def chunk(tag, data):
        body = tag + data
        return (
            struct.pack(">I", len(data))
            + body
            + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    with open(path, "wb") as fh:
        fh.write(b"\x89PNG\r\n\x1a\n")
        fh.write(chunk(b"IHDR", ihdr))
        fh.write(chunk(b"IDAT", zlib.compress(raw, 9)))
        fh.write(chunk(b"IEND", b""))


if __name__ == "__main__":
    main()
