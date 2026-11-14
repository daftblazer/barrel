/* Sends key taps to an X display (the game's, inside gamescope) through XTest, for oracle
 * runs: xkey KEY [KEY ...], each an X keysym name (Return, Escape, Down, z, ...), "down:KEY" / "up:KEY"
 * (held keys), "click:N" (mouse button N: Ryza DX confirms with the left button) or "sleep:MS".
 * Build: cc -O2 -o xkey xkey.c -lX11 -lXtst */
#include <X11/Xlib.h>
#include <X11/extensions/XTest.h>
#include <X11/keysym.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    Display *d = XOpenDisplay(NULL);
    if (!d) {
        fprintf(stderr, "xkey: cannot open display\n");
        return 1;
    }
    for (int i = 1; i < argc; i++) {
        if (strncmp(argv[i], "sleep:", 6) == 0) {
            usleep(atoi(argv[i] + 6) * 1000);
            continue;
        }
        if (strncmp(argv[i], "click:", 6) == 0) {
            unsigned b = (unsigned)atoi(argv[i] + 6);
            XTestFakeButtonEvent(d, b, True, 0);
            XFlush(d);
            usleep(80 * 1000);
            XTestFakeButtonEvent(d, b, False, 0);
            XFlush(d);
            usleep(250 * 1000);
            continue;
        }
        int held = strncmp(argv[i], "down:", 5) == 0 ? 1 : strncmp(argv[i], "up:", 3) == 0 ? 0 : -1;
        if (held >= 0) {
            KeySym hs = XStringToKeysym(argv[i] + (held ? 5 : 3));
            if (hs != NoSymbol) {
                XTestFakeKeyEvent(d, XKeysymToKeycode(d, hs), held ? True : False, 0);
                XFlush(d);
            }
            continue;
        }
        KeySym s = XStringToKeysym(argv[i]);
        if (s == NoSymbol) {
            fprintf(stderr, "xkey: unknown key %s\n", argv[i]);
            continue;
        }
        KeyCode k = XKeysymToKeycode(d, s);
        XTestFakeKeyEvent(d, k, True, 0);
        XFlush(d);
        usleep(80 * 1000);
        XTestFakeKeyEvent(d, k, False, 0);
        XFlush(d);
        usleep(250 * 1000);
    }
    XCloseDisplay(d);
    return 0;
}
