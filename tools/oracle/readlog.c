/* A preload library for oracle runs: logs every read Wine makes from a file whose path
 * contains READLOG_MATCH (default "Series"; the voice banks), with the time, the path, the
 * offset and the length, to READLOG_FILE. Wine's unix side reads files with pread64 (and read); paths come
 * from the open calls caught here or, failing that, /proc/self/fd.
 *   cc -O2 -shared -fPIC -o work/readlog.so tools/oracle/readlog.c -ldl
 *   LD_PRELOAD=$PWD/work/readlog.so READLOG_FILE=$PWD/work/oracle/reads.log ...
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#define MAX_FD 65536
static char *names[MAX_FD];
static FILE *out;

static const char *match(void) {
    const char *m = getenv("READLOG_MATCH");
    return m ? m : "Series";
}

static void note(int fd, const char *path) {
    if (fd < 0 || fd >= MAX_FD || !path || !strstr(path, match())) return;
    free(names[fd]);
    names[fd] = strdup(path);
}

static void logread(int fd, off_t off, size_t len) {
    if (fd < 0 || fd >= MAX_FD) return;
    if (!names[fd]) {
        /* Opened through a call not caught here (open64, a syscall): ask /proc. */
        char link[64], path[4096];
        snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
        ssize_t n = readlink(link, path, sizeof path - 1);
        if (n <= 0) return;
        path[n] = 0;
        if (!strstr(path, match())) return;
        names[fd] = strdup(path);
    }
    if (!out) {
        const char *f = getenv("READLOG_FILE");
        out = fopen(f ? f : "/dev/null", "a");
        if (!out) return;
        setvbuf(out, NULL, _IOLBF, 0);
    }
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    fprintf(out, "%ld.%03ld %s %lld %zu\n", (long)t.tv_sec, t.tv_nsec / 1000000, names[fd], (long long)off, len);
}

int openat(int dir, const char *path, int flags, ...) {
    static int (*real)(int, const char *, int, ...);
    if (!real) real = dlsym(RTLD_NEXT, "openat");
    mode_t mode = 0;
    if (flags & O_CREAT) { va_list a; va_start(a, flags); mode = va_arg(a, mode_t); va_end(a); }
    int fd = real(dir, path, flags, mode);
    note(fd, path);
    return fd;
}

int open(const char *path, int flags, ...) {
    static int (*real)(const char *, int, ...);
    if (!real) real = dlsym(RTLD_NEXT, "open");
    mode_t mode = 0;
    if (flags & O_CREAT) { va_list a; va_start(a, flags); mode = va_arg(a, mode_t); va_end(a); }
    int fd = real(path, flags, mode);
    note(fd, path);
    return fd;
}

ssize_t pread64(int fd, void *buf, size_t len, off_t off) {
    static ssize_t (*real)(int, void *, size_t, off_t);
    if (!real) real = dlsym(RTLD_NEXT, "pread64");
    logread(fd, off, len);
    return real(fd, buf, len, off);
}

ssize_t pread(int fd, void *buf, size_t len, off_t off) {
    static ssize_t (*real)(int, void *, size_t, off_t);
    if (!real) real = dlsym(RTLD_NEXT, "pread");
    logread(fd, off, len);
    return real(fd, buf, len, off);
}

ssize_t read(int fd, void *buf, size_t len) {
    static ssize_t (*real)(int, void *, size_t);
    if (!real) real = dlsym(RTLD_NEXT, "read");
    if (fd > 2 && len >= 64) logread(fd, lseek(fd, 0, SEEK_CUR), len);
    return real(fd, buf, len);
}
