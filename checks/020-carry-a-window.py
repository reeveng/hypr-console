"""Held with L2, a shoulder carries the window rather than leaving it."""

FEATURE = "carry"
SINCE = "2026-08-25"


def here(pad, seen):
    pad.trigger("l2", 1.0)
    pad.press("r1")
    seen.settle()
    assert seen.dispatches() == ['hl.dsp.window.move({workspace = "+1"})'], \
        "it asked for %s" % seen.dispatches()


def device(pad, seen):
    """The window comes along. Which is the only way to move one without a
    keyboard, so it is worth knowing on the machine rather than in a model."""
    if not seen.windows():
        raise AssertionError("nothing is open to carry")
    here = seen.workspace()
    carried = len(seen.windows())
    pad.trigger("l2", 1.0)
    pad.press("r1")
    pad.trigger("l2", 0.0)
    seen.settle(1.2)
    there = seen.workspace()
    assert there != here, "it did not move"
    assert len(seen.windows()) == carried, "a window was lost on the way"
    pad.trigger("l2", 1.0)
    pad.press("l1")
    pad.trigger("l2", 0.0)
    seen.settle(1.2)
