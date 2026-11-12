/* Sends key taps to an X display (the game's, inside gamescope) through XTest, for oracle
 * runs: xkey KEY [KEY ...], each an X keysym name (Return, Escape, Down, z, ...) or "sleep:MS".
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
