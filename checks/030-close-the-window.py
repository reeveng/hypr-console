"""The top right paddle closes what is in front of you."""

FEATURE = "close"
SINCE = "2026-08-24"


def here(pad, seen):
    pad.press("right-paddle-top")
    seen.settle()
    assert seen.dispatches() == ["hl.dsp.window.close()"], \
        "it asked for %s" % seen.dispatches()


def device(pad, seen):
    """Only ever run with something open that can be lost without regret.

    Counted on the workspace being looked at, because that is the one the
    paddle acts on. Counted across the machine, a window sitting on some other
    workspace is enough to say there was something to close.
    """
    if not seen.windows_here():
        assert seen.open(), "nothing would open on the device"
    before = seen.windows_here()
    pad.press("right-paddle-top")
    seen.settle(1.2)
    now = seen.windows_here()
    assert now < before, "%d window(s) before and %d after" % (before, now)
