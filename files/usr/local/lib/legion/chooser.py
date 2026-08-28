"""One chooser at a time, and the door that opened it closes it.

A chooser takes the controller while it is up: the buttons stop being the
desktop's and become move the highlight, confirm, and back out. It gives them
back when it goes. Two choosers at once and that is no longer true of either of
them. The second to open takes the profile the first was relying on, and the
first to close hands the desktop's buttons back while the other is still on
screen, so what you are looking at is being driven by the buttons of something
you cannot see.

It is invisible while it happens. Two of the same chooser are drawn in the same
place, so backing out of one leaves you looking at what appears to be the same
chooser that just ignored you. Pressing back harder is the natural thing to try
and it does nothing, because every press is closing a real chooser and there is
another behind it.

Nothing stopped it before: the menu is on a button, on a paddle and on a key,
the settings are on a button and on the bar, and every one of those roads
started a new process that knew nothing about the others.

A second chooser is not turned away, though, because the bar can be tapped
while one is up and a tap that does nothing at all is a bar that looks broken.
The one on screen goes and the new one takes its place. Asked for through the
same door it came out of, it goes and nothing replaces it: the icon that
brought a panel out is the icon that puts it away, which is the only way a
finger has of closing anything the settings icons open.

The lock is a file in the session's own runtime directory, held open for as
long as the process lives. The kernel drops it when the process ends however it
ends, so a chooser that is killed outright leaves nothing behind to clear, and
waiting for the lock rather than for the process is what puts the two in order:
the one going hands the controller back before it dies, and the lock is not
free until it has.

The name written beside the pid is the door, not the program. Two of the bar's
icons are the same program at different tabs, and tapping one while the other
is up should move the panel rather than close it.
"""
import fcntl
import os
import signal
import time
from pathlib import Path

_held = None

PATIENCE = 100      # tries at the lock while the last chooser leaves
BREATH = 0.02       # seconds between them


def where():
    """The lock's file, under whatever this session calls its runtime."""
    runtime = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
    return Path(runtime) / "legion" / "chooser.lock"


def take(handle):
    """Try the lock once, without waiting for it."""
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        return False
    return True


def holder(handle):
    """Who has it, and which door they came out of."""
    handle.seek(0)
    pid, _, name = handle.read().strip().partition(" ")
    try:
        return int(pid), name
    except ValueError:
        return 0, ""


def alone(name=""):
    """True if this process may be the chooser, False if it may not.

    False means there is nothing more to do: either the panel that was up has
    been closed by this call, which is what the same door asked twice means, or
    something else is holding the screen and will not let go.

    The handle is kept in a module variable rather than returned, because a
    lock is released when the last handle to it is closed and a caller who is
    not expecting to be holding anything has no reason to keep one.
    """
    global _held
    if _held is not None:
        return True

    path = where()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        handle = open(path, "a+")
    except OSError:
        return True         # nowhere to keep a lock is not a reason to refuse

    if not take(handle):
        pid, holding = holder(handle)
        # A second chooser inside one process is a program asking twice, and
        # there is no taking the screen from yourself.
        if pid in (0, os.getpid()):
            handle.close()
            return False

        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass            # it went while this was being read

        for _ in range(PATIENCE):
            if take(handle):
                break
            time.sleep(BREATH)
        else:
            handle.close()  # it will not go, and two of them is worse
            return False

        if holding == name:
            handle.close()
            return False

    handle.seek(0)
    handle.truncate()
    handle.write("%d %s" % (os.getpid(), name))
    handle.flush()
    _held = handle
    return True
