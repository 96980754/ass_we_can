/* 开发自测辅助：把指定 X11 窗口渲染成 PPM 图片。
 * 用法: xshot <window-id> <out.ppm>
 */
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: %s <window-id> <out.ppm>\n", argv[0]);
        return 2;
    }
    Display *dpy = XOpenDisplay(NULL);
    if (!dpy) {
        fprintf(stderr, "cannot open display\n");
        return 1;
    }
    Window win = (Window)strtoul(argv[1], NULL, 0);
    XWindowAttributes attr;
    if (!XGetWindowAttributes(dpy, win, &attr)) {
        fprintf(stderr, "bad window\n");
        return 1;
    }
    XImage *img = XGetImage(dpy, win, 0, 0, attr.width, attr.height, AllPlanes, ZPixmap);
    if (!img) {
        fprintf(stderr, "XGetImage failed\n");
        return 1;
    }
    FILE *f = fopen(argv[2], "wb");
    if (!f) {
        fprintf(stderr, "cannot write %s\n", argv[2]);
        return 1;
    }
    fprintf(f, "P6\n%d %d\n255\n", attr.width, attr.height);
    for (int y = 0; y < attr.height; y++) {
        for (int x = 0; x < attr.width; x++) {
            unsigned long p = XGetPixel(img, x, y);
            fputc((p >> 16) & 0xff, f);
            fputc((p >> 8) & 0xff, f);
            fputc(p & 0xff, f);
        }
    }
    fclose(f);
    XDestroyImage(img);
    XCloseDisplay(dpy);
    return 0;
}
