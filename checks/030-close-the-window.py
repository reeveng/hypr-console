"""The top right paddle closes what is in front of you."""

FEATURE = "close"
SINCE = "2026-08-24"


def here(pad, seen):
    pad.press("right-paddle-top")
    seen.settle()
    assert seen.dispatches() == ["hl.dsp.window.close()"], \
        "it asked for %s" % seen.dispatches()


def device(pad, seen):
    """Only ever run with something open that can be lost without regret."""
    before = seen.windows()
    if not before:
        raise AssertionError("nothing is open to close")
    pad.press("right-paddle-top")
    seen.settle(1.2)
    assert len(seen.windows()) < len(before), "the window is still there"
