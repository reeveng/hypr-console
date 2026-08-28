"""A trigger short of held moves you, and leaves the window where it was."""

FEATURE = "carry"
SINCE = "2026-08-25"


def here(pad, seen):
    pad.trigger("l2", 0.4)
    pad.press("r1")
    seen.settle()
    assert seen.dispatches() == ['hl.dsp.focus({workspace = "+1"})'], \
        "it asked for %s" % seen.dispatches()


def device(pad, seen):
    if not seen.windows():
        raise AssertionError("nothing is open to leave behind")
    pad.trigger("l2", 0.4)
    pad.press("r1")
    pad.trigger("l2", 0.0)
    seen.settle(1.2)
    assert not seen.windows(), "the window came along and should not have"
    pad.press("l1")
    seen.settle(1.0)
