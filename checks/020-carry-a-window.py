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
    keyboard, so it is worth knowing on the machine rather than in a model.

    It used to count every client on the device and ask that the number had
    not changed, which is true of a window that came and a window that stayed
    alike: moving one between workspaces does not make or destroy it. So this
    passed green with the trigger doing nothing at all, and said so in a check
    whose whole subject is the trigger. Count where the window is instead.
    """
    if not seen.windows_here():
        assert seen.open(), "nothing would open on the device"
    where = seen.workspace()
    set_out = seen.windows_here()
    pad.trigger("l2", 1.0)
    pad.press("r1")
    pad.trigger("l2", 0.0)
    seen.settle(1.2)
    there = seen.workspace()
    arrived = seen.windows_here()
    pad.trigger("l2", 1.0)
    pad.press("l1")
    pad.trigger("l2", 0.0)
    seen.settle(1.2)
    assert there != where, "it did not move"
    assert arrived == set_out, \
        "%d window(s) set out and %d arrived" % (set_out, arrived)
