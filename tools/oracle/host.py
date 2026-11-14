#!/usr/bin/env python3
"""Oracle host: launches an oracle run as its own descendant (a child subreaper, so the game
stays one after run.sh exits) and serves reads of the game's memory, which ptrace_scope 1
allows only to ancestors.

    tools/oracle/host.py INSTALL_DIR PROTON_DIR      (run in the background)

Commands, one per line, written to work/oracle/cmd (a FIFO the host makes):
    read ADDR LEN OUT     write LEN bytes from ADDR (hex) of the game's memory to OUT
    write ADDR HEX        write the bytes HEX (hex digits) to ADDR (hex) of the game's memory
                          (only ever an oracle run's copy of a save: the run's own state)
    pid                   write the game's pid to work/oracle/pid
    quit                  stop the run (tools/oracle/stop.sh) and exit
Each command's completion is appended to work/oracle/done as "ok <command>" or "err ...".
"""
import ctypes
import os
import subprocess
import sys

PR_SET_CHILD_SUBREAPER = 36
here = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
work = os.path.join(here, "work", "oracle")
os.makedirs(work, exist_ok=True)
libc = ctypes.CDLL(None, use_errno=True)
libc.prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0)

fifo = os.path.join(work, "cmd")
if os.path.exists(fifo):
    os.remove(fifo)
os.mkfifo(fifo)
done = open(os.path.join(work, "done"), "a", buffering=1)
subprocess.run([os.path.join(here, "tools", "oracle", "run.sh"), sys.argv[1], sys.argv[2]], check=True)


# The game's executable (run.sh's ORACLE_EXE), without its extension.
exe_name = os.path.splitext(os.environ.get("ORACLE_EXE", "Atelier_Escha_and_Logy_EN.exe"))[0]


def game_pid():
    """The process with the game's executable mapped at its image base."""
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            with open(f"/proc/{p}/maps") as m:
                if any(l.startswith("140000000-") and exe_name in l for l in m):
                    return int(p)
        except OSError:
            continue
    return None


while True:
    # Reap any orphans handed to us.
    try:
        while os.waitpid(-1, os.WNOHANG)[0] > 0:
            pass
    except ChildProcessError:
        pass
    with open(fifo) as f:
        for line in f:
            words = line.split()
            if not words:
                continue
            try:
                if words[0] == "quit":
                    subprocess.run([os.path.join(here, "tools", "oracle", "stop.sh")])
                    done.write("ok quit\n")
                    sys.exit(0)
                pid = game_pid()
                if pid is None:
                    done.write(f"err no game: {line.strip()}\n")
                    continue
                if words[0] == "pid":
                    open(os.path.join(work, "pid"), "w").write(str(pid))
                elif words[0] == "read":
                    addr, n, out = int(words[1], 16), int(words[2]), words[3]
                    with open(f"/proc/{pid}/mem", "rb") as mem:
                        mem.seek(addr)
                        data = mem.read(n)
                    open(out, "wb").write(data)
                elif words[0] == "write":
                    addr, data = int(words[1], 16), bytes.fromhex(words[2])
                    with open(f"/proc/{pid}/mem", "r+b", buffering=0) as mem:
                        mem.seek(addr)
                        mem.write(data)
                done.write(f"ok {line.strip()}\n")
            except Exception as e:  # report and keep serving
                done.write(f"err {e}: {line.strip()}\n")
