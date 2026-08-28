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
    """The count that answers this is the one for the workspace being looked
    at. Every client on the machine is a different question, and the window
    this check opens itself would answer that one wrongly for ever.

    Going back is done before the assertion rather than after it, so that a
    failure leaves the desk where it found it. It did not, and the next check
    closed the active window on a workspace that no longer had one.
    """
    if not seen.windows_here():
        assert seen.open(), "nothing would open on the device"
    pad.trigger("l2", 0.4)
    pad.press("r1")
    pad.trigger("l2", 0.0)
    seen.settle(1.2)
    came = seen.windows_here()
    pad.press("l1")
    seen.settle(1.0)
    assert not came, "%d window(s) came along and none should have" % came
